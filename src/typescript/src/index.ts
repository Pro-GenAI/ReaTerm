/**
 * Native ReaTerm adapter. Never construct a shell command or use shell:true.
 * Optional OpenAI decisions may veto, but never extend Rust permissions.
 */
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, isAbsolute, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export interface ReaTermOptions {
  root?: string;
  binary?: string;
  timeoutMs?: number;
  decisionModel?: string;
  apiKey?: string;
  baseURL?: string;
}

export class ReaTermError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ReaTermError";
  }
}

const MAX_CAPTURE_BYTES = 2 * 1024 * 1024;

function resolveBinary(explicit?: string): string {
  if (explicit) return explicit;
  if (process.env.REATERM_BIN) return process.env.REATERM_BIN;
  // Source-checkout convenience.
  const root = resolve(dirname(fileURLToPath(import.meta.url)), "../../../");
  for (const profile of ["release", "debug"]) {
    const binaryName = process.platform === "win32" ? "reaterm.exe" : "reaterm";
    const candidate = resolve(root, "rust", "target", profile, binaryName);
    if (existsSync(candidate)) return candidate;
  }
  return "reaterm";
}

export class ReaTerm {
  readonly root: string;
  readonly binary: string;
  readonly timeoutMs: number;
  readonly decisionModel?: string;
  readonly apiKey?: string;
  readonly baseURL?: string;

  constructor(options: ReaTermOptions = {}) {
    this.root = resolve(options.root ?? ".");
    this.binary = resolveBinary(options.binary);
    this.timeoutMs = options.timeoutMs ?? 15000;
    this.decisionModel = options.decisionModel ?? process.env.REATERM_DECISION_MODEL;
    this.apiKey = options.apiKey ?? process.env.REATERM_OPENAI_API_KEY ?? process.env.OPENAI_API_KEY;
    this.baseURL = options.baseURL ?? process.env.REATERM_OPENAI_BASE_URL ?? process.env.OPENAI_BASE_URL;
  }

  private async modelAllows(argv: readonly string[]): Promise<void> {
    if (!this.decisionModel) return;
    if (!this.apiKey) {
      throw new ReaTermError("Decision model requires REATERM_OPENAI_API_KEY or OPENAI_API_KEY");
    }
    let OpenAI: typeof import("openai").default;
    try {
      ({ default: OpenAI } = await import("openai"));
    } catch {
      throw new ReaTermError("Install the official openai npm package for decision models");
    }
    let result: string | null | undefined;
    try {
      const client = new OpenAI({
        apiKey: this.apiKey,
        ...(this.baseURL ? { baseURL: this.baseURL } : {}),
      });
      const completion = await client.chat.completions.create({
        model: this.decisionModel,
        messages: [
          {
            role: "system",
            content:
              "You are a conservative read-only command veto checker. " +
              "Reply with exactly ALLOW or DENY. ALLOW only if argv is clearly " +
              "non-mutating. Reject shell syntax, writes, code execution, unsafe " +
              "flags or uncertainty. The native executor independently uses a " +
              "stricter allowlist; you cannot grant new permissions.",
          },
          { role: "user", content: JSON.stringify(argv) },
        ],
      });
      result = completion.choices[0]?.message?.content;
    } catch (err) {
      throw new ReaTermError("Decision model failed: " + String(err));
    }
    if (result?.trim() !== "ALLOW") {
      throw new ReaTermError("Command denied by optional decision model");
    }
  }

  /** Execute a native read-only command; returns stdout and throws on denial. */
  async run(command: string, ...args: string[]): Promise<string> {
    if (!command || ![command, ...args].every((arg) => typeof arg === "string")) {
      throw new ReaTermError("command and each argument must be strings");
    }
    await this.modelAllows([command, ...args]);
    const argv = ["--root", this.root, command, ...args];
    return new Promise<string>((resolveResult, rejectResult) => {
      const child = spawn(this.binary, argv, {
        shell: false,
        stdio: ["ignore", "pipe", "pipe"],
        timeout: this.timeoutMs,
      });
      const stdout: Buffer[] = [];
      const stderr: Buffer[] = [];
      let byteCount = 0;
      let finished = false;
      const fail = (message: string) => {
        if (!finished) {
          finished = true;
          rejectResult(new ReaTermError(message));
        }
      };
      child.on("error", (err) => fail("Unable to launch ReaTerm: " + err.message));
      const receive = (chunks: Buffer[]) => (buf: Buffer) => {
        byteCount += buf.length;
        if (byteCount > MAX_CAPTURE_BYTES) {
          child.kill("SIGKILL");
          fail("ReaTerm output exceeds client capture limit");
          return;
        }
        chunks.push(buf);
      };
      child.stdout.on("data", receive(stdout));
      child.stderr.on("data", receive(stderr));
      child.on("close", (code, signal) => {
        if (finished) return;
        finished = true;
        if (signal) {
          rejectResult(new ReaTermError("ReaTerm terminated with signal " + signal));
        } else if (code !== 0) {
          rejectResult(
            new ReaTermError(Buffer.concat(stderr).toString("utf8").trim() || "ReaTerm denied command")
          );
        } else {
          resolveResult(Buffer.concat(stdout).toString("utf8"));
        }
      });
    });
  }
}

/**
 * Split one convenient command string into argv, without shell execution.
 * Supports whitespace, single/double quotes and backslash escaping only.
 * Shell operators/substitutions are deliberately not accepted here; use the
 * ReaTerm class API for exact literal arguments, including unusual filenames.
 */
function parseCommandLine(commandLine: string): string[] {
  if (typeof commandLine !== "string" || !commandLine.trim()) {
    throw new ReaTermError("commandLine must be a nonempty string");
  }
  if (/[\0\r\n;&|<>$\x60]/.test(commandLine)) {
    throw new ReaTermError("Shell operators and substitutions are not supported");
  }

  const tokens: string[] = [];
  let word = "";
  let started = false;
  let quote: "'" | '"' | null = null;
  for (let i = 0; i < commandLine.length; i++) {
    const char = commandLine[i];
    if (quote === "'") {
      if (char === "'") quote = null;
      else word += char;
    } else if (char === "\\") {
      const next = commandLine[i + 1];
      if (next === undefined) {
        throw new ReaTermError("Invalid command line: trailing escape");
      }
      if (quote === '"' && next !== '"' && next !== "\\") {
        word += "\\";
      } else {
        word += next;
        i++;
      }
      started = true;
    } else if (char === '"' || (char === "'" && quote === null)) {
      if (quote === char) quote = null;
      else if (quote === null) quote = char;
      else word += char;
      started = true;
    } else if (quote === null && /\s/.test(char)) {
      if (started) {
        tokens.push(word);
        word = "";
        started = false;
      }
    } else {
      word += char;
      started = true;
    }
  }
  if (quote !== null) throw new ReaTermError("Invalid command line: unterminated quote");
  if (started) tokens.push(word);
  if (!tokens[0]) throw new ReaTermError("commandLine must start with a command");
  return tokens;
}

/** Execute a single read-only command string using the native ReaTerm binary. */
export async function run(commandLine: string, options: ReaTermOptions = {}): Promise<string> {
  const [command, ...args] = parseCommandLine(commandLine);
  return new ReaTerm(options).run(command, ...args);
}

# ReaTerm Website Development and Deployment

Static, responsive project website for **https://reaterm.pro-genai.com/**.
The document root is this directory, docs/website/. There is **no build
step, dependency installation, Node server, or backend required**.

## Preview locally

The website also supports `make -C docs/website setup` (no dependencies)
and `make -C docs/website test`. The root `make test` runs the website
tests after Rust, Python, and TypeScript pass.

From the repository root:

```bash
cd docs/website
python -m http.server 8000
```

Open **http://localhost:8000/** in your browser. Press Ctrl+C in the
terminal to stop the server. If Python is installed as python3, use:

```bash
python3 -m http.server 8000
```

Or run it from the repository root without changing directories:

```bash
python -m http.server 8000 --directory docs/website
```

For local viewing, http.server is sufficient. **It is a development server,
not a production hosting setup.**

## Production hosting at reaterm.pro-genai.com

Serve the **contents of docs/website/** as the site root (not the entire
repository). Upload everything in this directory, preserving paths.
Configure your static host with:

- **Build command:** none / leave blank
- **Publish/output directory:** docs/website (for hosts connected to repo)
- **Index file:** index.html
- **Custom domain:** reaterm.pro-genai.com
- **HTTPS:** enable TLS certificate and redirect HTTP to HTTPS
- **404 page:** 404.html
- **Canonical URL:** https://reaterm.pro-genai.com/

For Cloudflare Pages, for example: connect the GitHub repository, use
**no framework**, **no build command**, publish **docs/website**, then add
reaterm.pro-genai.com as the project's custom domain and complete the
platform's DNS verification. **Creating these files does not automatically
configure hosting, DNS, Cloudflare Pages, or a deployment.**

For GitHub Pages with a custom Actions deployment, publish docs/website as
the artifact root. The CNAME file is included for setups that use it.
Do not assume branch-source GitHub Pages can serve arbitrary nested
docs/website/ folders: use an Actions deployment or another static host.

## File layout

```text
docs/website/
  index.html                Landing page
  styles.css                Responsive design
  app.js                    Copy buttons + accessible integration tabs
  favicon.svg               Tab icon
  assets/reaterm-cover.svg  Local marketing image for link previews
  robots.txt                Search crawling policy
  sitemap.xml               Canonical sitemap
  CNAME                     Intended custom host name
  404.html                  Useful not-found page
  test_site.py              stdlib-only static and HTTP smoke tests
  README.md                 Public-facing website overview
```

All site resources are relative paths, so the site works on localhost and
after deployment. No external JavaScript libraries, analytics, or remote
fonts are required. The GitHub and registry links lead off-site.

## Test locally or in CI

From the repository root:

```bash
python -m unittest discover -s docs/website -p "test_*.py" -v
```

The tests check required assets, internal links, markup/accessibility,
configured URLs, safe/no-dependency JavaScript, and that a local HTTP
server serves the page, stylesheet and script. The repository CI workflows are currently disabled; run these tests locally.

## Package links and availability

The website documents Linux x86-64/ARM64, macOS Intel/Apple Silicon and
Windows x86-64, with the Windows thread-token scope clearly stated.
Keep platform requirements synchronized with docs/DEVELOPMENT.md.

The website links to the canonical Rust, Python and TypeScript source
folders under https://github.com/Pro-GenAI/ReaTerm/tree/main/src/.
It also includes links to the expected PyPI and npm registry listings:
https://pypi.org/project/reaterm/ and
https://www.npmjs.com/package/reaterm.
**Their published availability is not verified.** Source installation
remains the documented method. Do not claim either package has been
published until release automation confirms it.

The Python distribution metadata and npm package.json both point to the
intended website URL. The site URL will only work after DNS and hosting
have been configured.

## Python provider tools on the website

The homepage includes the Python OpenAI Chat Completions / Responses and Anthropic Messages tool schemas and links to `docs/TOOL_INTEGRATIONS.md`. Keep example imports synchronized with `src/python/reaterm/tools/` and the Python package README. These tool schemas are Python-only, require explicit execution and response submission, and do not change the native Rust allowlist. The JavaScript npm package supports the `run("ls -al")` convenience function but not these Python tool exports. Disclose that returning stdout to a provider may expose files from the caller-selected root. Do not add third-party scripts or SDKs to the static website.

## Website README content policy

The root README, both language-package READMEs, and
`docs/website/README.md` are public-facing product introductions. Keep
them limited to benefits, concise user-usage examples, essential
security limitations, and relevant links. Place contributor commands,
testing, release instructions, hosting and deployment details in
`docs/DEVELOPMENT.md`, `docs/TESTING.md` or this guide instead.

## Content and safety

Keep the site synchronized with README.md, docs/THREAT_MODEL.md and
AGENTS.md. ReaTerm only limits its own native process; it cannot block
output redirection performed by a parent shell. Optional decision models
are only implemented by the Python and TypeScript wrappers and can veto
requests, not authorize commands denied by Rust. Use no fake performance
metrics, false publication claims, or guarantees about unprotected launchers.

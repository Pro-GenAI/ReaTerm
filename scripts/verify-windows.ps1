# ReaTerm Windows dev/CI runner (PowerShell 5.1+).
# Set up and test Rust -> Python -> TypeScript -> website; stop on first failure.
param(
    [ValidateSet("setup", "test", "verify")]
    [string]$Mode = "verify",
    [ValidateSet("all", "rust", "python", "typescript", "website")]
    [string]$Language = "all"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$rust = Join-Path $root "src\rust"
$python = Join-Path $root "src\python"
$ts = Join-Path $root "src\typescript"
$site = Join-Path $root "docs\website"
$binary = Join-Path $rust "target\release\reaterm.exe"
$pythonVenv = Join-Path $python ".venv\Scripts\python.exe"

# Do not send real commands to an external model during regression tests.
$env:REATERM_DECISION_MODEL = ""

function Run-Checked([string]$Program, [string[]]$Arguments, [string]$Directory) {
    Push-Location $Directory
    try {
        Write-Host "[$Directory] $Program $($Arguments -join ' ')" -ForegroundColor Cyan
        & $Program @Arguments
        if ($LASTEXITCODE -ne 0) {
            throw "Command failed (exit code $LASTEXITCODE): $Program $($Arguments -join ' ')"
        }
    } finally {
        Pop-Location
    }
}

function Assert-Binary {
    if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) {
        throw "Missing native Rust binary: $binary. Run Rust setup first."
    }
}

function Setup-Rust {
    Run-Checked "cargo" @("build", "--release") $rust
}

function Test-Rust {
    Run-Checked "cargo" @("fmt", "--all", "--", "--check") $rust
    Run-Checked "cargo" @("clippy", "--all-targets", "--", "-D", "warnings") $rust
    Run-Checked "cargo" @("test", "--all") $rust
}

function Setup-Python {
    Run-Checked "python" @("-m", "venv", ".venv") $python
    Run-Checked $pythonVenv @("-m", "pip", "install", "--disable-pip-version-check", "-e", ".") $python
}

function Test-Python {
    Assert-Binary
    $env:REATERM_BIN = $binary
    Run-Checked $pythonVenv @("-m", "unittest", "discover", "-s", "tests", "-p", "test_*.py", "-v") $python
}

function Setup-TypeScript {
    Run-Checked "npm.cmd" @("install", "--no-audit", "--no-fund") $ts
}

function Test-TypeScript {
    Assert-Binary
    $env:REATERM_BIN = $binary
    Run-Checked "npm.cmd" @("run", "check") $ts
}

function Setup-Website {
    Write-Host "Static website: no dependencies to install."
}

function Test-Website {
    Run-Checked "python" @("-m", "unittest", "discover", "-s", ".", "-p", "test_*.py", "-v") $site
}

$stages = @(
    @{ Name = "Rust"; Setup = "Setup-Rust"; Test = "Test-Rust" },
    @{ Name = "Python"; Setup = "Setup-Python"; Test = "Test-Python" },
    @{ Name = "TypeScript"; Setup = "Setup-TypeScript"; Test = "Test-TypeScript" },
    @{ Name = "Website"; Setup = "Setup-Website"; Test = "Test-Website" }
)

try {
    foreach ($stage in $stages) {
        if ($Language -ne "all" -and $Language -ne $stage.Name.ToLowerInvariant()) {
            continue
        }
        if ($Mode -ne "test") {
            Write-Host "==> $($stage.Name): setup" -ForegroundColor Green
            & ($stage.Setup)
        }
        if ($Mode -ne "setup") {
            Write-Host "==> $($stage.Name): test" -ForegroundColor Green
            & ($stage.Test)
        }
    }
    Write-Host "ReaTerm Windows $Mode completed successfully" -ForegroundColor Green
} catch {
    Write-Error "Windows $Mode stopped at first failure: $_"
    exit 1
}

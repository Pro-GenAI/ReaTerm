.DEFAULT_GOAL := help

NPM ?= npm
REATERM_BIN ?= $(abspath ../rust/target/release/reaterm)

.PHONY: help setup test verify

help:
	@printf '%s\n' 'TypeScript commands: make setup | make test | make verify'
	@printf '%s\n' 'Standalone TypeScript verification also needs the Rust release binary.'

# The repository does not currently commit package-lock.json.
ifeq ($(OS),Windows_NT)
# GNU Make on Windows uses the native PowerShell runner. The shell scripts
# and venv locations differ from POSIX; never silently skip native tests.
setup:
	powershell.exe -NoProfile -ExecutionPolicy Bypass -File "../../scripts/verify-windows.ps1" -Mode setup -Language typescript

test:
	powershell.exe -NoProfile -ExecutionPolicy Bypass -File "../../scripts/verify-windows.ps1" -Mode test -Language typescript

verify:
	powershell.exe -NoProfile -ExecutionPolicy Bypass -File "../../scripts/verify-windows.ps1" -Mode verify -Language typescript
else
setup:
	$(NPM) install --no-audit --no-fund

test:
	@test -x "$(REATERM_BIN)" || { echo 'Native Rust binary missing; run make -C src/rust setup first (from the repository root)' >&2; exit 1; }
	@REATERM_DECISION_MODEL= REATERM_BIN="$(REATERM_BIN)" $(NPM) run check

verify:
	@$(MAKE) setup
	@$(MAKE) test

endif

.DEFAULT_GOAL := help

CARGO ?= cargo

.PHONY: help setup test verify

help:
	@printf '%s\n' 'Rust commands: make setup | make test | make verify'

# Build the trusted native binary for the Python and Node integration tests.
# No global Cargo installation or change to the user's PATH is required.
ifeq ($(OS),Windows_NT)
# GNU Make on Windows uses the native PowerShell runner. The shell scripts
# and venv locations differ from POSIX; never silently skip native tests.
setup:
	powershell.exe -NoProfile -ExecutionPolicy Bypass -File "../../scripts/verify-windows.ps1" -Mode setup -Language rust

test:
	powershell.exe -NoProfile -ExecutionPolicy Bypass -File "../../scripts/verify-windows.ps1" -Mode test -Language rust

verify:
	powershell.exe -NoProfile -ExecutionPolicy Bypass -File "../../scripts/verify-windows.ps1" -Mode verify -Language rust
else
setup:
	$(CARGO) build --release

test:
	$(CARGO) fmt --all -- --check
	$(CARGO) clippy --all-targets -- -D warnings
	$(CARGO) test --all

verify:
	@$(MAKE) setup
	@$(MAKE) test

endif

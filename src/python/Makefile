.DEFAULT_GOAL := help

PYTHON ?= python3
VENV_DIR ?= .venv
VENV_PYTHON := $(VENV_DIR)/bin/python
REATERM_BIN ?= $(abspath ../rust/target/release/reaterm)

.PHONY: help setup test verify

help:
	@printf '%s\n' 'Python commands: make setup | make test | make verify'
	@printf '%s\n' 'Standalone Python verification also needs the Rust release binary.'

# Always install into an ignored virtualenv, not the system Python.
ifeq ($(OS),Windows_NT)
# GNU Make on Windows uses the native PowerShell runner. The shell scripts
# and venv locations differ from POSIX; never silently skip native tests.
setup:
	powershell.exe -NoProfile -ExecutionPolicy Bypass -File "../../scripts/verify-windows.ps1" -Mode setup -Language python

test:
	powershell.exe -NoProfile -ExecutionPolicy Bypass -File "../../scripts/verify-windows.ps1" -Mode test -Language python

verify:
	powershell.exe -NoProfile -ExecutionPolicy Bypass -File "../../scripts/verify-windows.ps1" -Mode verify -Language python
else
setup:
	@$(PYTHON) -c 'import sys; sys.exit("ReaTerm requires Python 3.11 or newer; set PYTHON to a supported interpreter") if sys.version_info < (3, 11) else None'
	$(PYTHON) -m venv "$(VENV_DIR)"
	@"$(VENV_PYTHON)" -c 'import sys; sys.exit("Virtualenv requires Python 3.11 or newer; remove it and rerun setup") if sys.version_info < (3, 11) else None'
	"$(VENV_PYTHON)" -m pip install --disable-pip-version-check --upgrade pip
	"$(VENV_PYTHON)" -m pip install --disable-pip-version-check -e '.[mcp-test]'

test:
	@test -x "$(VENV_PYTHON)" || { echo 'Python virtualenv missing; run make setup first' >&2; exit 1; }
	@test -x "$(REATERM_BIN)" || { echo 'Native Rust binary missing; run make -C src/rust setup first (from the repository root)' >&2; exit 1; }
	@REATERM_DECISION_MODEL= REATERM_BIN="$(REATERM_BIN)" "$(VENV_PYTHON)" -m unittest discover -s tests -p 'test_*.py' -v
	@REATERM_DECISION_MODEL= REATERM_BIN="$(REATERM_BIN)" "$(VENV_PYTHON)" -m pytest -q mcp_tests

verify:
	@$(MAKE) setup
	@$(MAKE) test

endif

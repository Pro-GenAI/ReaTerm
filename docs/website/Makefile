.DEFAULT_GOAL := help

PYTHON ?= python3

.PHONY: help setup test verify

help:
	@printf '%s\n' 'Website commands: make setup | make test | make verify'

ifeq ($(OS),Windows_NT)
# GNU Make on Windows uses the native PowerShell runner. The shell scripts
# and venv locations differ from POSIX; never silently skip native tests.
setup:
	powershell.exe -NoProfile -ExecutionPolicy Bypass -File "../../scripts/verify-windows.ps1" -Mode setup -Language website

test:
	powershell.exe -NoProfile -ExecutionPolicy Bypass -File "../../scripts/verify-windows.ps1" -Mode test -Language website

verify:
	powershell.exe -NoProfile -ExecutionPolicy Bypass -File "../../scripts/verify-windows.ps1" -Mode verify -Language website
else
setup:
	@echo 'Static website: no dependencies to install'

test:
	$(PYTHON) -m unittest discover -s . -p 'test_*.py' -v

verify:
	@$(MAKE) setup
	@$(MAKE) test

endif

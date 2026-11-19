.DEFAULT_GOAL := help

# Override these paths in tests or when embedding ReaTerm in another checkout.
RUST_DIR ?= src/rust
PYTHON_DIR ?= src/python
TYPESCRIPT_DIR ?= src/typescript
WEBSITE_DIR ?= docs/website

.PHONY: help setup test verify

help:
	@printf '%s\n' 'ReaTerm development commands:' \
	  '  make setup   Install/build Rust, Python, and TypeScript prerequisites' \
	  '  make test    Run their tests and the website tests (run setup first)' \
	  '  make verify  Set up AND test each language sequentially, stopping on failure'

# These are deliberately recipes, NOT parallel prerequisites: setup cannot
# start language N+1 if an earlier language installation failed.
setup:
	@$(MAKE) -C "$(RUST_DIR)" setup
	@$(MAKE) -C "$(PYTHON_DIR)" setup
	@$(MAKE) -C "$(TYPESCRIPT_DIR)" setup
	@$(MAKE) -C "$(WEBSITE_DIR)" setup

test:
	@$(MAKE) -C "$(RUST_DIR)" test
	@$(MAKE) -C "$(PYTHON_DIR)" test
	@$(MAKE) -C "$(TYPESCRIPT_DIR)" test
	@$(MAKE) -C "$(WEBSITE_DIR)" test

# CRITICAL: interleave setup and test per language. Any unsuccessful recipe
# stops make immediately, before any later language is installed or tested.
# Do not rewrite this as "verify: setup test" or parallel prerequisites.
verify:
	@$(MAKE) -C "$(RUST_DIR)" verify
	@$(MAKE) -C "$(PYTHON_DIR)" verify
	@$(MAKE) -C "$(TYPESCRIPT_DIR)" verify
	@$(MAKE) -C "$(WEBSITE_DIR)" verify

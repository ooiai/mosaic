SHELL := /bin/sh
.DEFAULT_GOAL := help

# Notes: Toolchain and workspace entrypoint variables. Override from the command line if needed.
GIT ?= git
CARGO ?= cargo
CLI_PATH ?= cli
CLI_PACKAGE ?= mosaic-cli
CLI_BIN ?= mosaic
REAL_CC_BIN ?= /usr/bin/cc
INSTALL_ROOT ?=
INSTALL_FLAGS ?= --locked
CARGO_LINKER_ENV := REAL_CC_BIN=$(REAL_CC_BIN) CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=$(CURDIR)/scripts/cc-linker-wrapper.sh

ifneq ($(strip $(INSTALL_ROOT)),)
INSTALL_ROOT_FLAG := --root $(INSTALL_ROOT)
else
INSTALL_ROOT_FLAG :=
endif

# Notes: Add, commit, and push only when the worktree has changes.
define git_push_if_needed
	@if [ -n "$$($(GIT) status --porcelain)" ]; then \
		$(GIT) add .; \
		$(GIT) commit -m "$(m)"; \
		$(GIT) push; \
	else \
		echo "No changes to commit"; \
	fi
endef

# Notes: Add and commit only when the worktree has changes.
define git_commit_if_needed
	@if [ -n "$$($(GIT) status --porcelain)" ]; then \
		$(GIT) add .; \
		$(GIT) commit -m "$(m)"; \
	else \
		echo "No changes to commit"; \
	fi
endef

# Notes: Fail early when a required variable is missing.
define require_arg
	@if [ -z "$(strip $($(1)))" ]; then \
		echo "Missing required variable: $(1)"; \
		echo "Usage: make $(2) $(1)=<value>"; \
		exit 1; \
	fi
endef

.PHONY: \
	help \
	install \
	uninstall \
	run \
	build \
	clean \
	check \
	test \
	verify \
	git-run \
	git-commit \
	git-rm-cache

help: ## Show available Make targets.
	@awk 'BEGIN { FS = ":.*## "; print "Available targets:\n" } /^[a-zA-Z0-9_-]+:.*## / { printf "  %-12s %s\n", $$1, $$2 }' $(MAKEFILE_LIST)
	@printf "\nUsage examples:\n"
	@printf "  make build\n"
	@printf "  make check\n"
	@printf "  make test\n"
	@printf "  make verify\n"
	@printf "  make run\n"
	@printf "  make install\n"
	@printf "  make install INSTALL_ROOT=/tmp/mosaic-test-root\n"
	@printf "  make uninstall INSTALL_ROOT=/tmp/mosaic-test-root\n"
	@printf "  make git-commit m=\"docs: update readme\"\n"
	@printf "  make git-run m=\"feat: improve tui\"\n"
	@printf "  make git-rm-cache f=path/to/file\n"

# Notes: Install the CLI binary from the workspace root.
# Usage: make install
# Usage: make install INSTALL_ROOT=/tmp/mosaic-test-root
# Usage: make install INSTALL_ROOT=/tmp/mosaic-test-root INSTALL_FLAGS="--offline --locked"
install: ## Install the CLI binary from the cli crate.
	make check
	@if $(CARGO) uninstall $(CLI_PACKAGE) $(INSTALL_ROOT_FLAG); then \
		:; \
	else \
		echo "$(CLI_PACKAGE) is not currently installed in the selected Cargo root"; \
	fi
	$(CARGO_LINKER_ENV) $(CARGO) install --path $(CLI_PATH) --force $(INSTALL_ROOT_FLAG) $(INSTALL_FLAGS)

# Notes: Uninstall the CLI package from Cargo's install root.
# Usage: make uninstall
# Usage: make uninstall INSTALL_ROOT=/tmp/mosaic-test-root
uninstall: ## Uninstall the CLI package from Cargo's install root.
	@if $(CARGO) uninstall $(CLI_PACKAGE) $(INSTALL_ROOT_FLAG); then \
		:; \
	else \
		echo "$(CLI_PACKAGE) is not currently installed in the selected Cargo root"; \
	fi

# Notes: Run the CLI locally without installing it.
# Usage: make run
run: ## Run the CLI from the workspace.
	$(CARGO_LINKER_ENV) $(CARGO) run -p $(CLI_PACKAGE) --bin $(CLI_BIN)

# Notes: Build the CLI crate without installing it globally.
# Usage: make build
build: ## Build the CLI crate.
	$(CARGO_LINKER_ENV) $(CARGO) build -p $(CLI_PACKAGE)

# Notes: Remove Cargo build artifacts for the whole workspace.
# Usage: make clean
clean: ## Clean workspace build artifacts.
	$(CARGO) clean

# Notes: Run a lightweight workspace validation pass.
# Usage: make check
check: ## Run workspace checks.
	$(CARGO_LINKER_ENV) $(CARGO) check --workspace

# Notes: Run the workspace test suite.
# Usage: make test
test: ## Run workspace tests.
	$(CARGO_LINKER_ENV) $(CARGO) test --workspace

# Notes: Run the default verification chain before handoff.
# Usage: make verify
verify: ## Run build, check, and test in sequence.
	$(MAKE) build
	$(MAKE) check
	$(MAKE) test

# Notes: Add, commit, and push when the worktree has changes.
# Usage: make git-run m="message"
git-run: ## Add, commit, and push if the worktree has changes. Usage: make git-run m="message"
	$(call require_arg,m,git-run)
	$(call git_push_if_needed)

# Notes: Add and commit locally when the worktree has changes.
# Usage: make git-commit m="message"
git-commit: ## Add and commit if the worktree has changes. Usage: make git-commit m="message"
	$(call require_arg,m,git-commit)
	$(call git_commit_if_needed)

# Notes: Remove a file from the git index without deleting the working tree file.
# Usage: make git-rm-cache f=path/to/file
git-rm-cache: ## Remove a file from the git index only. Usage: make git-rm-cache f=path/to/file
	$(call require_arg,f,git-rm-cache)
	$(GIT) rm --cached $(f)

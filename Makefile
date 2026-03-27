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
	package \
	clean \
	check \
	test \
	test-unit \
	test-integration \
	test-real \
	test-golden \
	ci-fast \
	ci-real \
	smoke \
	release-check \
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
	@printf "  make test-unit\n"
	@printf "  make test-integration\n"
	@printf "  make test-golden\n"
	@printf "  MOSAIC_REAL_TESTS=1 make test-real\n"
	@printf "  make ci-fast\n"
	@printf "  make smoke\n"
	@printf "  make verify\n"
	@printf "  make release-check\n"
	@printf "  make package\n"
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
	$(MAKE) check
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

# Notes: Build a release binary bundle with docs, examples, and env template under dist/.
# Usage: make package
package: ## Build a release bundle under dist/.
	@set -eu; \
	pkg_dir="dist/$(CLI_BIN)-$$($(GIT) rev-parse --short HEAD)"; \
	rm -rf "$$pkg_dir" "$$pkg_dir.tar.gz"; \
	mkdir -p "$$pkg_dir/bin"; \
	$(CARGO_LINKER_ENV) $(CARGO) build -p $(CLI_PACKAGE) --release; \
	cp target/release/$(CLI_BIN) "$$pkg_dir/bin/"; \
	cp README.md LICENSE .env.example "$$pkg_dir/"; \
	cp -R docs examples "$$pkg_dir/"; \
	tar -czf "$$pkg_dir.tar.gz" -C dist "$$(basename "$$pkg_dir")"; \
	printf "Created %s\n" "$$pkg_dir.tar.gz"

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

# Notes: Run fast unit-style tests for libraries and binaries only.
# Usage: make test-unit
test-unit: ## Run fast unit-focused tests.
	$(CARGO_LINKER_ENV) $(CARGO) test --workspace --lib --bins

# Notes: Run crate integration tests under crates/*/tests and cli/tests surfaces.
# Usage: make test-integration
test-integration: ## Run local integration tests.
	$(CARGO_LINKER_ENV) $(CARGO) test --workspace --tests

# Notes: Run golden example verification from setup to inspect in an isolated workspace.
# Usage: make test-golden
test-golden: ## Run golden example and docs command verification.
	./scripts/test-golden-examples.sh

# Notes: Run real integration tests when MOSAIC_REAL_TESTS=1 and any required secrets are present.
# Usage: MOSAIC_REAL_TESTS=1 make test-real
test-real: ## Run gated real integration tests.
	./scripts/test-real-integrations.sh

# Notes: Default CI lane for pull requests and local pre-merge checks.
# Usage: make ci-fast
ci-fast: ## Run the fast CI verification lane.
	$(MAKE) check
	$(MAKE) test-unit
	$(MAKE) test-integration
	$(MAKE) test-golden

# Notes: Optional CI lane for real service checks with secrets and local daemons.
# Usage: MOSAIC_REAL_TESTS=1 make ci-real
ci-real: ## Run the gated real-service CI lane.
	$(MAKE) test-real

# Notes: Run the release smoke path in an isolated temporary workspace.
# Usage: make smoke
smoke: ## Run the release smoke script in a temporary workspace.
	./scripts/release-smoke.sh

# Notes: Run the release checklist gate: docs/artifacts check, workspace verification, and smoke.
# Usage: make release-check
release-check: ## Run delivery artifact checks, verify, and smoke.
	./scripts/verify-delivery-artifacts.sh
	$(MAKE) verify
	$(MAKE) smoke

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

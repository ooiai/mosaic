# Variables
GIT := git
PNPM := pnpm
CARGO := cargo
DOCKER := docker
CD := cd

DESKTOP_PATH := ./apps/desktop
WEB_PATH := ./apps/web



# Function to check if there are changes to commit
define git_push_if_needed
	@if [ -n "$$($(GIT) status --porcelain)" ]; then \
		$(GIT) add .; \
		$(GIT) commit -m "$(m)"; \
		$(GIT) push; \
	else \
		echo "No changes to commit"; \
	fi
endef

define git_commit_if_needed
	@if [ -n "$$($(GIT) status --porcelain)" ]; then \
		$(GIT) add .; \
		$(GIT) commit -m "$(m)"; \
	else \
		echo "No changes to commit"; \
	fi
endef

# Git run add commit push
git-run:
	$(call git_push_if_needed)

# Git run add commit push
git-commit:
	$(call git_commit_if_needed)

# backend watch commands
# clean:
# 	@echo "Cleaning backend in $(BACKEND_PATH)..."
# 	cd $(BACKEND_PATH) && $(CARGO) clean

# Desktop start dev server.
# Usage: make desktop
desktop:
	@echo "===> Desktop start dev server."
	$(CD) $(DESKTOP_PATH) && $(PNPM) dev

# Web start dev server.
# Usage: make web
web:
	@echo "===> Web start dev server."
	$(CD) $(WEB_PATH) && $(PNPM) dev

# Rust CLI workspace tests.
# Usage: make cli-test
cli-test:
	@echo "===> Rust CLI workspace tests."
	cd cli && cargo test --workspace

# Rust CLI quality gate (fast local guardrails).
# Usage: make cli-quality
cli-quality:
	@echo "===> Rust CLI quality gate (check + clippy + tests)."
	cd cli && cargo check -p mosaic-cli
	cd cli && cargo clippy -p mosaic-cli -- -D warnings
	cd cli && cargo test -p mosaic-cli --test command_surface
	cd cli && cargo test -p mosaic-cli

# Rust CLI JSON contract gate (envelopes + schema snapshots + help snapshots + error codes).
# Usage: make cli-json-contract
cli-json-contract:
	@echo "===> Rust CLI JSON contract gate."
	cd cli && cargo test -p mosaic-cli --test error_codes
	cd cli && cargo test -p mosaic-cli --test json_contract
	cd cli && cargo test -p mosaic-cli --test json_contract_modules
	cd cli && cargo test -p mosaic-cli --test help_snapshot

# Rust CLI full regression.
# Usage: make cli-regression
cli-regression:
	@echo "===> Rust CLI full regression."
	cd cli && ./scripts/run_regression_suite.sh

# Rust CLI beta readiness gate.
# Usage: make cli-beta-check
cli-beta-check:
	@echo "===> Rust CLI beta readiness gate."
	cd cli && ./scripts/beta_release_check.sh

# Rust CLI beta package.
# Usage: make cli-beta-package v=v0.2.0-beta.1
cli-beta-package:
	@if [ -z "$(v)" ]; then \
		echo "error: missing version. usage: make cli-beta-package v=v0.2.0-beta.1"; \
		exit 1; \
	fi
	@echo "===> Rust CLI beta package ($(v))."
	cd cli && ./scripts/package_beta.sh --version "$(v)"

# Rust CLI release asset package (single target).
# Usage: make cli-release-assets v=v0.2.0-beta.5 t=aarch64-apple-darwin
cli-release-assets:
	@if [ -z "$(v)" ] || [ -z "$(t)" ]; then \
		echo "error: missing args. usage: make cli-release-assets v=v0.2.0-beta.5 t=aarch64-apple-darwin"; \
		exit 1; \
	fi
	@echo "===> Rust CLI release asset ($(v), $(t))."
	cd cli && ./scripts/package_release_asset.sh --version "$(v)" --target "$(t)"

# Generate Homebrew/Scoop manifests from release assets.
# Usage: make cli-release-manifests v=v0.2.0-beta.5 assets=dist/v0.2.0-beta.5 out=dist/v0.2.0-beta.5
cli-release-manifests:
	@if [ -z "$(v)" ] || [ -z "$(assets)" ]; then \
		echo "error: missing args. usage: make cli-release-manifests v=v0.2.0-beta.5 assets=dist/v0.2.0-beta.5 [out=dist/v0.2.0-beta.5]"; \
		exit 1; \
	fi
	@echo "===> Rust CLI release manifests ($(v))."
	cd cli && ./scripts/update_distribution_manifests.sh --version "$(v)" --assets-dir "$(assets)" $(if $(out),--output-dir "$(out)",)

# Docs static acceptance gate (docs.js syntax + local links).
# Usage: make docs-check
docs-check:
	@echo "===> Docs acceptance checks."
	bash site/scripts/check_docs.sh

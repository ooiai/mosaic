# Variables
GIT := git
PNPM := pnpm
CARGO := cargo
DOCKER := docker
CD := cd

MACOS_PATH := ./apps/macos
WEB_PATH := ./apps/web
CLI_PATH := ./cli
SERVER_PATH := ./server

.PHONY: \
	clean \
	cli-clean \
	git-run \
	git-commit \
	help \
	desktop \
	macos-dev \
	macos-dev-wait \
	macos-build \
	macos-test \
	macos-package \
	web \
	web-build \
	web-lint \
	web-typecheck \
	cli-build \
	cli-build-release \
	cli-run \
	cli-install \
	cli-test \
	cli-quality \
	cli-json-contract \
	cli-regression \
	cli-beta-check \
	cli-release-tooling-smoke \
	cli-release-install-smoke \
	cli-beta-package \
	cli-release-assets \
	cli-release-manifests \
	cli-release-notes \
	cli-release-prepare \
	cli-release-verify \
	cli-release-verify-archives \
	cli-release-publish-check \
	docs-check



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

# Mosaic clean (Rust/Cargo + macOS/Swift + web/Node).
# Usage: make clean
clean:
	@echo "Cleaning build artifacts..."
	rm -rf node_modules
	cd $(SERVER_PATH) && cargo clean
	cd $(CLI_PATH) && cargo clean
	cd $(CLI_PATH) && rm -rf dist
	cd $(MACOS_PATH) && swift package clean
	cd $(MACOS_PATH) && rm -rf dist
	cd $(MACOS_PATH) && rm -rf .build
	cd $(WEB_PATH) && rm -rf dist
	cd $(WEB_PATH) && rm -rf node_modules


# Desktop start dev server.
# Usage: make desktop
desktop:
	@$(MAKE) macos-dev

# macOS-native app start dev server.
# Usage: make macos-dev
macos-dev:
	@echo "===> macOS app package and open (non-blocking)."
	cd cli && cargo build --release -p mosaic-cli
	SWIFT_CONFIGURATION=debug APP_BUILD=dev SKIP_CLI_BUILD=1 ./apps/macos/scripts/package_app.sh
	open -n "$(PWD)/apps/macos/dist/Mosaic.app"

# macOS-native app start dev server and wait for app exit.
# Usage: make macos-dev-wait
macos-dev-wait:
	@echo "===> macOS app package and open (wait for exit)."
	cd cli && cargo build --release -p mosaic-cli
	SWIFT_CONFIGURATION=debug APP_BUILD=dev SKIP_CLI_BUILD=1 ./apps/macos/scripts/package_app.sh
	open -n -W "$(PWD)/apps/macos/dist/Mosaic.app"

# macOS-native app release build.
# Usage: make macos-build
macos-build:
	@echo "===> macOS app release build."
	$(CD) $(MACOS_PATH) && swift build -c release

# macOS-native app tests.
# Usage: make macos-test
macos-test:
	@echo "===> macOS app tests."
	$(CD) $(MACOS_PATH) && swift test

# Package macOS-native app bundle.
# Usage: make macos-package
macos-package:
	@echo "===> Package macOS app bundle."
	./apps/macos/scripts/package_app.sh

# Web start dev server.
# Usage: make web
web:
	@echo "===> Web start dev server."
	$(CD) $(WEB_PATH) && $(PNPM) dev

# Web production build.
# Usage: make web-build
web-build:
	@echo "===> Web production build."
	$(CD) $(WEB_PATH) && $(PNPM) build

# Web lint.
# Usage: make web-lint
web-lint:
	@echo "===> Web lint."
	$(CD) $(WEB_PATH) && $(PNPM) lint

# Web typecheck.
# Usage: make web-typecheck
web-typecheck:
	@echo "===> Web typecheck."
	$(CD) $(WEB_PATH) && $(PNPM) typecheck

# Rust CLI clean
# Usage: make cli-clean
cli-clean:
	@echo "===> Rust CLI clean."
	cd cli && cargo clean -p mosaic-cli

# Rust CLI debug build.
# Usage: make cli-build
cli-build:
	@echo "===> Rust CLI debug build."
	cd cli && cargo build -p mosaic-cli

# Rust CLI release build.
# Usage: make cli-build-release
cli-build-release:
	@echo "===> Rust CLI release build."
	cd cli && cargo build --release -p mosaic-cli

# Rust CLI run current source.
# Usage: make cli-run [args='--help']
cli-run:
	@echo "===> Rust CLI run current source."
	cd cli && cargo run -p mosaic-cli -- $(if $(args),$(args),--help)

# Rust CLI install current source to Cargo bin.
# Usage: make cli-install
cli-install:
	@echo "===> Rust CLI install current source."
	@BIN_PATH="$${CARGO_HOME:-$$HOME/.cargo}/bin/mosaic"; \
	if [ -e "$$BIN_PATH" ]; then \
		echo "===> Removing existing CLI at $$BIN_PATH"; \
		rm -f "$$BIN_PATH"; \
	fi
	cd cli && cargo install --path crates/mosaic-cli --force

# Rust CLI uninstall from Cargo bin.
# Usage: make cli-uninstall
cli-uninstall:
	@echo "===> Rust CLI uninstall current source."
	@BIN_PATH="$${CARGO_HOME:-$$HOME/.cargo}/bin/mosaic"; \
	if [ -e "$$BIN_PATH" ]; then \
		echo "===> Removing existing CLI at $$BIN_PATH"; \
		rm -f "$$BIN_PATH"; \
	fi

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

# Rust CLI release tooling smoke (release verifiers + dry-run summary path).
# Usage: make cli-release-tooling-smoke [v=v0.2.0-beta.6]
cli-release-tooling-smoke:
	@echo "===> Rust CLI release tooling smoke."
	cd cli && ./scripts/release_tooling_smoke.sh $(if $(v),--version "$(v)",)

# Rust CLI installer smoke (install.sh local assets + release-only behavior).
# Usage: make cli-release-install-smoke [v=v0.2.0-beta.6]
cli-release-install-smoke:
	@echo "===> Rust CLI release install smoke."
	cd cli && ./scripts/release_install_smoke.sh $(if $(v),--version "$(v)",)

# Rust CLI beta package.
# Usage: make cli-beta-package v=v0.2.0-beta.6
cli-beta-package:
	@if [ -z "$(v)" ]; then \
		echo "error: missing version. usage: make cli-beta-package v=v0.2.0-beta.6"; \
		exit 1; \
	fi
	@echo "===> Rust CLI beta package ($(v))."
	cd cli && ./scripts/package_beta.sh --version "$(v)"

# Rust CLI release asset package (single target).
# Usage: make cli-release-assets v=v0.2.0-beta.6 t=aarch64-apple-darwin
cli-release-assets:
	@if [ -z "$(v)" ] || [ -z "$(t)" ]; then \
		echo "error: missing args. usage: make cli-release-assets v=v0.2.0-beta.6 t=aarch64-apple-darwin"; \
		exit 1; \
	fi
	@echo "===> Rust CLI release asset ($(v), $(t))."
	cd cli && ./scripts/package_release_asset.sh --version "$(v)" --target "$(t)"

# Generate Homebrew/Scoop manifests from release assets.
# Usage: make cli-release-manifests v=v0.2.0-beta.6 assets=dist/v0.2.0-beta.6 out=dist/v0.2.0-beta.6
cli-release-manifests:
	@if [ -z "$(v)" ] || [ -z "$(assets)" ]; then \
		echo "error: missing args. usage: make cli-release-manifests v=v0.2.0-beta.6 assets=dist/v0.2.0-beta.6 [out=dist/v0.2.0-beta.6]"; \
		exit 1; \
	fi
	@echo "===> Rust CLI release manifests ($(v))."
	cd cli && ./scripts/update_distribution_manifests.sh --version "$(v)" --assets-dir "$(assets)" $(if $(out),--output-dir "$(out)",)

# Generate release notes draft from WORKLOG.
# Usage:
#   make cli-release-notes v=v0.2.0-beta.6
#   make cli-release-notes v=v0.2.0-beta.6 from=2026-03-01T00:00:00Z max=30 out=docs/release-notes-v0.2.0-beta.6.md
cli-release-notes:
	@if [ -z "$(v)" ]; then \
		echo "error: missing version. usage: make cli-release-notes v=v0.2.0-beta.6 [from=ISO8601] [max=20] [out=docs/release-notes-<version>.md]"; \
		exit 1; \
	fi
	@echo "===> Rust CLI release notes draft ($(v))."
	cd cli && ./scripts/release_notes_from_worklog.sh --version "$(v)" $(if $(from),--from-date "$(from)",) $(if $(max),--max-entries "$(max)",) $(if $(out),--out "$(out)",)

# One-command local release prepare (checks + notes + single-target asset + optional manifests).
# Usage:
#   make cli-release-prepare v=v0.2.0-beta.6
#   make cli-release-prepare v=v0.2.0-beta.6 t=aarch64-apple-darwin max=30 notes=docs/release-notes-v0.2.0-beta.6.md summary=reports/release-prepare.json
#   make cli-release-prepare v=v0.2.0-beta.6 assets=../release-assets out=../release-assets skip_check=1
#   make cli-release-prepare v=v0.2.0-beta.6 assets=../release-assets out=../release-assets skip_verify=1
#   make cli-release-prepare v=v0.2.0-beta.6 assets=../release-assets out=../release-assets skip_archive_check=1
cli-release-prepare:
	@if [ -z "$(v)" ]; then \
		echo "error: missing version. usage: make cli-release-prepare v=v0.2.0-beta.6 [t=<target>] [from=ISO8601] [max=20] [notes=<path>] [assets=<dir>] [out=<dir>] [summary=<path>] [skip_check=1] [skip_archive_check=1] [skip_verify=1] [dry_run=1]"; \
		exit 1; \
	fi
	@echo "===> Rust CLI release prepare ($(v))."
	cd cli && ./scripts/release_prepare.sh --version "$(v)" $(if $(t),--target "$(t)",) $(if $(from),--notes-from-date "$(from)",) $(if $(max),--notes-max-entries "$(max)",) $(if $(notes),--notes-out "$(notes)",) $(if $(assets),--assets-dir "$(assets)",) $(if $(out),--output-dir "$(out)",) $(if $(summary),--summary-out "$(summary)",) $(if $(filter 1 true yes,$(skip_check)),--skip-check,) $(if $(filter 1 true yes,$(skip_archive_check)),--skip-archive-check,) $(if $(filter 1 true yes,$(skip_verify)),--skip-verify,) $(if $(filter 1 true yes,$(dry_run)),--dry-run,)

# Verify release assets in a directory (archives/checksums/manifests/installers/notes).
# Usage:
#   make cli-release-verify v=v0.2.0-beta.6 assets=../release-assets
#   make cli-release-verify v=v0.2.0-beta.6 assets=../release-assets notes=docs/release-notes-v0.2.0-beta.6.md
cli-release-verify:
	@if [ -z "$(v)" ] || [ -z "$(assets)" ]; then \
		echo "error: missing args. usage: make cli-release-verify v=v0.2.0-beta.6 assets=../release-assets [notes=<path>] [json=1]"; \
		exit 1; \
	fi
	@echo "===> Rust CLI release asset verification ($(v))."
	cd cli && ./scripts/release_verify_assets.sh --version "$(v)" --assets-dir "$(assets)" $(if $(notes),--notes "$(notes)",) $(if $(filter 1 true yes,$(json)),--json,)

# Verify archive internal contents in a directory (binary/docs/layout).
# Usage:
#   make cli-release-verify-archives v=v0.2.0-beta.6 assets=../release-assets
cli-release-verify-archives:
	@if [ -z "$(v)" ] || [ -z "$(assets)" ]; then \
		echo "error: missing args. usage: make cli-release-verify-archives v=v0.2.0-beta.6 assets=../release-assets [json=1]"; \
		exit 1; \
	fi
	@echo "===> Rust CLI archive content verification ($(v))."
	cd cli && ./scripts/release_verify_archives.sh --version "$(v)" --assets-dir "$(assets)" $(if $(filter 1 true yes,$(json)),--json,)

# Verify published GitHub release assets by tag.
# Usage:
#   make cli-release-publish-check v=v0.2.0-beta.6
#   make cli-release-publish-check v=v0.2.0-beta.6 repo=ooiai/mosaic json=1
cli-release-publish-check:
	@if [ -z "$(v)" ]; then \
		echo "error: missing version. usage: make cli-release-publish-check v=v0.2.0-beta.6 [repo=owner/repo] [token_env=GITHUB_TOKEN] [notes=<path>] [json=1]"; \
		exit 1; \
	fi
	@echo "===> Rust CLI published release verification ($(v))."
	cd cli && ./scripts/release_publish_check.sh --version "$(v)" $(if $(repo),--repo "$(repo)",) $(if $(token_env),--token-env "$(token_env)",) $(if $(notes),--notes "$(notes)",) $(if $(filter 1 true yes,$(json)),--json,)

# Docs static acceptance gate (docs.js syntax + local links).
# Usage: make docs-check
docs-check:
	@echo "===> Docs acceptance checks."
	bash site/scripts/check_docs.sh

# Show common developer commands.
# Usage: make help
help:
	@echo "Common commands:"
	@echo "  make macos-dev        # Package and open the macOS app without blocking the terminal"
	@echo "  make macos-dev-wait   # Package and open the macOS app, then wait for it to exit"
	@echo "  make macos-build      # Build the macOS app in release mode"
	@echo "  make macos-test       # Run macOS app tests"
	@echo "  make macos-package    # Build Mosaic.app and Mosaic-macOS.zip"
	@echo "  make web              # Run the web dev server"
	@echo "  make web-build        # Build the web app"
	@echo "  make web-lint         # Lint the web app"
	@echo "  make web-typecheck    # Typecheck the web app"
	@echo "  make cli-build        # Build the Rust CLI in debug mode"
	@echo "  make cli-build-release # Build the Rust CLI in release mode"
	@echo "  make cli-run          # Run the current Rust CLI source (default: --help)"
	@echo "  make cli-install      # Install the current Rust CLI source to Cargo bin"
	@echo "  make cli-test         # Run Rust CLI workspace tests"
	@echo "  make cli-quality      # Run Rust CLI quality gates"
	@echo "  make cli-json-contract # Run Rust CLI JSON contract tests"

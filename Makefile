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

# Rust CLI full regression.
# Usage: make cli-regression
cli-regression:
	@echo "===> Rust CLI full regression."
	cd cli && ./scripts/run_regression_suite.sh

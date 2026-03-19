# Variables
GIT := git
PNPM := pnpm
CARGO := cargo
DOCKER := docker
CD := cd

CLI_PATH := ./cli

.PHONY: \
	git-run \
	git-commit \


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

# Git run: add, commit, push if there are changes
# Usage: make git-run m="Your commit message"
git-run:
	$(call git_push_if_needed)

# Git commit: add and commit if there are changes, but do not push
# Usage: make git-commit m="Your commit message"
git-commit:
	$(call git_commit_if_needed)

# Git rm cache: remove a file from git cache
# Usage: make git-rm-cache f=path/to/file
git-rm-cache:
	$(GIT) rm --cached $(f)

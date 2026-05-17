.PHONY: all build release run test lint fmt clean help

# Default target
all: build

build: ## Build the project in debug mode
	cargo build

release: ## Build the project in release mode
	cargo build --release

run: ## Run the project
	cargo run

test: ## Run tests
	cargo test

lint: ## Run clippy and check formatting
	cargo clippy -- -D warnings
	cargo fmt --all -- --check

fmt: ## Format code
	cargo fmt --all

clean: ## Remove build artifacts
	cargo clean

venv: ## Create virtual environment
	python3 -m venv venv
	./venv/bin/pip install djlint

install-djlint: venv ## Install python djlint in venv for parity testing

fetch-test-data: ## Fetch external templates for parity testing
	./scripts/fetch_test_data.sh

compare-lint: ## Compare lint results between djlint and djlintr
	python3 scripts/compare_lint.py

compare-reformat: ## Compare reformat results between djlint and djlintr
	python3 scripts/compare_reformat.py

help: ## Show this help message
	@echo "Usage: make [target]"
	@echo ""
	@echo "Targets:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-10s\033[0m %s\n", $$1, $$2}'

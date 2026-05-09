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

help: ## Show this help message
	@echo "Usage: make [target]"
	@echo ""
	@echo "Targets:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-10s\033[0m %s\n", $$1, $$2}'

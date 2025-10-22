.PHONY: help build test test-unit test-integration lint fmt check run run-release clean

BINARY := raven

help:
	@echo "🐦 Project Raven CLI"
	@echo "===================="
	@echo ""
	@echo "Core commands:"
	@echo "  make build          Build the release binary"
	@echo "  make test           Run all tests"
	@echo "  make lint           Run clippy with -D warnings"
	@echo "  make fmt            Format the codebase with rustfmt"
	@echo "  make check          Run cargo check"
	@echo "  make run            Run the server with cargo run"
	@echo "  make run-release    Run the optimized release binary"
	@echo "  make clean          Remove target artifacts"

build:
	cargo build --release

test:
	cargo test --all

test-unit:
	cargo test --test unit

test-integration:
	cargo test --test integration

lint:
	cargo clippy --all-targets --all-features -- -D warnings

fmt:
	cargo fmt --all

check:
	cargo check --all-targets

run:
	cargo run --bin $(BINARY)

run-release: build
	./target/release/$(BINARY)

clean:
	cargo clean

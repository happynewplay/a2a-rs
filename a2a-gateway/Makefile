# A2A Gateway Makefile

.PHONY: help build test clean run dev docker lint fmt check coverage docs

# Default target
help:
	@echo "Available targets:"
	@echo "  build     - Build the project in release mode"
	@echo "  test      - Run all tests"
	@echo "  clean     - Clean build artifacts"
	@echo "  run       - Run the gateway with default config"
	@echo "  dev       - Run the gateway in development mode"
	@echo "  docker    - Build Docker image"
	@echo "  lint      - Run clippy linter"
	@echo "  fmt       - Format code"
	@echo "  check     - Check code without building"
	@echo "  coverage  - Generate test coverage report"
	@echo "  docs      - Generate documentation"

# Build targets
build:
	cargo build --release

build-dev:
	cargo build

# Test targets
test:
	cargo test

test-unit:
	cargo test --lib

test-integration:
	cargo test --test integration_test

test-verbose:
	cargo test -- --nocapture

# Development targets
run: build
	./target/release/a2a-gateway --config gateway.yaml

dev:
	cargo run -- --config gateway.yaml --log-level debug

# Docker targets
docker:
	docker build -t a2a-gateway:latest .

docker-run:
	docker run -p 8080:8080 -p 8081:8081 -p 9090:9090 a2a-gateway:latest

# Code quality targets
lint:
	cargo clippy -- -D warnings

fmt:
	cargo fmt

fmt-check:
	cargo fmt --check

check:
	cargo check

# Coverage target (requires cargo-tarpaulin)
coverage:
	cargo tarpaulin --out html --output-dir coverage

# Documentation
docs:
	cargo doc --no-deps --open

# Benchmarks (if any)
bench:
	cargo bench

# Clean targets
clean:
	cargo clean
	rm -rf coverage/

# Install development dependencies
install-deps:
	cargo install cargo-tarpaulin
	cargo install cargo-watch

# Watch for changes and rebuild
watch:
	cargo watch -x "build"

watch-test:
	cargo watch -x "test"

# Load testing (requires wrk)
load-test:
	@echo "Starting load test..."
	@echo "Make sure the gateway is running on localhost:8080"
	wrk -t12 -c400 -d30s http://localhost:8080/health

load-test-post:
	@echo "Starting POST load test..."
	@echo "Make sure the gateway is running on localhost:8080"
	wrk -t12 -c400 -d30s -s scripts/post.lua http://localhost:8080/tasks/send

# Setup development environment
setup:
	@echo "Setting up development environment..."
	rustup update
	rustup component add clippy rustfmt
	$(MAKE) install-deps
	@echo "Development environment ready!"

# Release preparation
prepare-release:
	$(MAKE) fmt
	$(MAKE) lint
	$(MAKE) test
	$(MAKE) build
	@echo "Release preparation complete!"

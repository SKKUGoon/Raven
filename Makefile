# Project Raven - Market Data Subscription Server
# Makefile for deployment and development operations

.PHONY: help build deploy start stop restart status logs clean backup test lint fmt check health bench bench-all bench-grpc bench-latency bench-memory bench-high-freq

# Default target
.DEFAULT_GOAL := help

# Colors for output
BLUE := \033[36m
GREEN := \033[32m
YELLOW := \033[33m
RED := \033[31m
NC := \033[0m # No Color

# Configuration
COMPOSE_FILE := docker/docker-compose.yml
DOCKERFILE := docker/Dockerfile
BACKUP_DIR := docker/backups
PROJECT_NAME := raven
RUST_VERSION := 1.75

## Help - Show available commands
help:
	@echo "$(BLUE)🐦 Project Raven - Market Data Subscription Server$(NC)"
	@echo "$(BLUE)=================================================$(NC)"
	@echo ""
	@echo "$(GREEN)Available commands:$(NC)"
	@echo ""
	@echo "$(YELLOW)Development:$(NC)"
	@echo "  build          Build the Rust application"
	@echo "  test           Run all tests"
	@echo "  lint           Run clippy linter"
	@echo "  fmt            Format code with rustfmt"
	@echo "  check          Run cargo check"
	@echo ""
	@echo "$(YELLOW)Benchmarks:$(NC)"
	@echo "  bench          Run basic benchmarks"
	@echo "  bench-all      Run all benchmarks with detailed output"
	@echo "  bench-grpc     Run gRPC streaming benchmarks"
	@echo "  bench-latency  Run latency benchmarks"
	@echo "  bench-memory   Run memory usage benchmarks"
	@echo "  bench-high-freq Run high frequency benchmarks"
	@echo ""
	@echo "$(YELLOW)Docker Operations:$(NC)"
	@echo "  docker-build   Build Docker image"
	@echo "  deploy         Deploy full stack with Docker Compose"
	@echo "  start          Start all services"
	@echo "  stop           Stop all services"
	@echo "  restart        Restart all services"
	@echo "  status         Show service status"
	@echo "  health         Check service health"
	@echo ""
	@echo "$(YELLOW)Monitoring & Logs:$(NC)"
	@echo "  logs           Show logs for all services"
	@echo "  logs-server    Show logs for market data server"
	@echo "  logs-influx    Show logs for InfluxDB"
	@echo "  logs-grafana   Show logs for Grafana"
	@echo ""
	@echo "$(YELLOW)Data Management:$(NC)"
	@echo "  backup         Create backup of data volumes"
	@echo "  restore        Restore from backup (requires BACKUP_NAME)"
	@echo "  clean          Clean up containers and networks"
	@echo "  clean-all      Clean up everything including volumes"
	@echo ""
	@echo "$(YELLOW)Utilities:$(NC)"
	@echo "  urls           Show service URLs"
	@echo "  shell          Open shell in running container"
	@echo "  db-shell       Open InfluxDB shell"
	@echo ""
	@echo "$(GREEN)Examples:$(NC)"
	@echo "  make deploy              # Deploy full stack"
	@echo "  make logs-server         # Show server logs"
	@echo "  make backup              # Create data backup"
	@echo "  make restore BACKUP_NAME=backup_20240101_120000"
	@echo "  make bench-all           # Run all performance benchmarks"
	@echo "  make bench-latency       # Run only latency benchmarks"

## Development Commands

# Build the Rust application
build:
	@echo "$(BLUE)🔨 Building Project Raven...$(NC)"
	cargo build --release
	@echo "$(GREEN)✅ Build completed$(NC)"

# Run all tests
test:
	@echo "$(BLUE)🧪 Running tests...$(NC)"
	cargo test --all
	@echo "$(GREEN)✅ Tests completed$(NC)"

# Run integration tests
test-integration:
	@echo "$(BLUE)🧪 Running integration tests...$(NC)"
	cargo test --test integration_tests
	@echo "$(GREEN)✅ Integration tests completed$(NC)"

# Run unit tests
test-unit:
	@echo "$(BLUE)🧪 Running unit tests...$(NC)"
	cargo test --test unit_tests
	@echo "$(GREEN)✅ Unit tests completed$(NC)"

# Run clippy linter
lint:
	@echo "$(BLUE)🔍 Running clippy linter...$(NC)"
	cargo clippy --all-targets --all-features -- -D warnings
	@echo "$(GREEN)✅ Linting completed$(NC)"

# Format code
fmt:
	@echo "$(BLUE)📝 Formatting code...$(NC)"
	cargo fmt --all
	@echo "$(GREEN)✅ Code formatted$(NC)"

# Check code without building
check:
	@echo "$(BLUE)🔍 Checking code...$(NC)"
	cargo check --all-targets
	@echo "$(GREEN)✅ Check completed$(NC)"

# Run benchmarks
bench:
	@echo "$(BLUE)⚡ Running benchmarks...$(NC)"
	cargo bench
	@echo "$(GREEN)✅ Benchmarks completed$(NC)"

# Run all benchmarks with proper output formatting
bench-all:
	@echo "$(BLUE)⚡ Running all benchmarks with detailed output...$(NC)"
	@echo "$(YELLOW)📊 Starting comprehensive benchmark suite...$(NC)"
	@echo ""
	cargo bench --bench grpc_streaming_benchmarks --bench latency_benchmarks --bench memory_usage_benchmarks --bench high_frequency_benchmarks
	@echo ""
	@echo "$(GREEN)✅ All benchmarks completed$(NC)"

# Run gRPC streaming benchmarks
bench-grpc:
	@echo "$(BLUE)⚡ Running gRPC streaming benchmarks...$(NC)"
	@echo "$(YELLOW)🌐 Testing gRPC streaming performance...$(NC)"
	cargo bench --bench grpc_streaming_benchmarks
	@echo "$(GREEN)✅ gRPC streaming benchmarks completed$(NC)"

# Run latency benchmarks
bench-latency:
	@echo "$(BLUE)⚡ Running latency benchmarks...$(NC)"
	@echo "$(YELLOW)⏱️  Testing system latency performance...$(NC)"
	cargo bench --bench latency_benchmarks
	@echo "$(GREEN)✅ Latency benchmarks completed$(NC)"

# Run memory usage benchmarks
bench-memory:
	@echo "$(BLUE)⚡ Running memory usage benchmarks...$(NC)"
	@echo "$(YELLOW)💾 Testing memory allocation and usage patterns...$(NC)"
	cargo bench --bench memory_usage_benchmarks
	@echo "$(GREEN)✅ Memory usage benchmarks completed$(NC)"

# Run high frequency benchmarks
bench-high-freq:
	@echo "$(BLUE)⚡ Running high frequency benchmarks...$(NC)"
	@echo "$(YELLOW)🚀 Testing high frequency data processing performance...$(NC)"
	cargo bench --bench high_frequency_benchmarks
	@echo "$(GREEN)✅ High frequency benchmarks completed$(NC)"

## Docker Operations

# Build Docker image
docker-build:
	@echo "$(BLUE)🐳 Building Docker image...$(NC)"
	docker build -f $(DOCKERFILE) -t $(PROJECT_NAME):latest .
	@echo "$(GREEN)✅ Docker image built$(NC)"

# Deploy full stack
deploy: docker-build
	@echo "$(BLUE)🚀 Deploying Project Raven full stack...$(NC)"
	@mkdir -p $(BACKUP_DIR)
	docker-compose -f $(COMPOSE_FILE) up -d
	@echo "$(YELLOW)⏳ Waiting for services to start...$(NC)"
	@sleep 30
	@$(MAKE) health
	@$(MAKE) urls
	@echo "$(GREEN)✅ Deployment completed$(NC)"

# Start services
start:
	@echo "$(BLUE)▶️  Starting services...$(NC)"
	docker-compose -f $(COMPOSE_FILE) up -d
	@echo "$(GREEN)✅ Services started$(NC)"

# Stop services
stop:
	@echo "$(BLUE)⏹️  Stopping services...$(NC)"
	docker-compose -f $(COMPOSE_FILE) down
	@echo "$(GREEN)✅ Services stopped$(NC)"

# Restart services
restart: stop start
	@echo "$(GREEN)✅ Services restarted$(NC)"

# Show service status
status:
	@echo "$(BLUE)📊 Service Status:$(NC)"
	@docker-compose -f $(COMPOSE_FILE) ps

# Check service health
health:
	@echo "$(BLUE)❤️  Checking service health...$(NC)"
	@echo ""
	@echo "$(YELLOW)InfluxDB:$(NC)"
	@if docker exec raven-influxdb influx ping 2>/dev/null; then \
		echo "$(GREEN)✅ InfluxDB is healthy$(NC)"; \
	else \
		echo "$(RED)❌ InfluxDB is not responding$(NC)"; \
	fi
	@echo ""
	@echo "$(YELLOW)Redis:$(NC)"
	@if docker exec raven-redis redis-cli ping 2>/dev/null | grep -q PONG; then \
		echo "$(GREEN)✅ Redis is healthy$(NC)"; \
	else \
		echo "$(RED)❌ Redis is not responding$(NC)"; \
	fi
	@echo ""
	@echo "$(YELLOW)Market Data Server:$(NC)"
	@if curl -sf http://localhost:8080/health >/dev/null 2>&1; then \
		echo "$(GREEN)✅ Market Data Server is healthy$(NC)"; \
	else \
		echo "$(RED)❌ Market Data Server is not responding$(NC)"; \
	fi
	@echo ""
	@echo "$(YELLOW)Prometheus:$(NC)"
	@if curl -sf http://localhost:9091/-/healthy >/dev/null 2>&1; then \
		echo "$(GREEN)✅ Prometheus is healthy$(NC)"; \
	else \
		echo "$(RED)❌ Prometheus is not responding$(NC)"; \
	fi
	@echo ""
	@echo "$(YELLOW)Grafana:$(NC)"
	@if curl -sf http://localhost:3000/api/health >/dev/null 2>&1; then \
		echo "$(GREEN)✅ Grafana is healthy$(NC)"; \
	else \
		echo "$(RED)❌ Grafana is not responding$(NC)"; \
	fi

## Monitoring & Logs

# Show logs for all services
logs:
	@echo "$(BLUE)📋 Showing logs for all services...$(NC)"
	docker-compose -f $(COMPOSE_FILE) logs -f

# Show logs for market data server
logs-server:
	@echo "$(BLUE)📋 Showing logs for Market Data Server...$(NC)"
	docker-compose -f $(COMPOSE_FILE) logs -f market-data-server

# Show logs for InfluxDB
logs-influx:
	@echo "$(BLUE)📋 Showing logs for InfluxDB...$(NC)"
	docker-compose -f $(COMPOSE_FILE) logs -f influxdb

# Show logs for Grafana
logs-grafana:
	@echo "$(BLUE)📋 Showing logs for Grafana...$(NC)"
	docker-compose -f $(COMPOSE_FILE) logs -f grafana

# Show logs for Prometheus
logs-prometheus:
	@echo "$(BLUE)📋 Showing logs for Prometheus...$(NC)"
	docker-compose -f $(COMPOSE_FILE) logs -f prometheus

# Show logs for Redis
logs-redis:
	@echo "$(BLUE)📋 Showing logs for Redis...$(NC)"
	docker-compose -f $(COMPOSE_FILE) logs -f redis

## Data Management

# Create backup
backup:
	@echo "$(BLUE)💾 Creating backup...$(NC)"
	@mkdir -p $(BACKUP_DIR)
	@TIMESTAMP=$$(date +%Y%m%d_%H%M%S) && \
	BACKUP_NAME="raven_backup_$$TIMESTAMP" && \
	mkdir -p "$(BACKUP_DIR)/$$BACKUP_NAME" && \
	echo "$(YELLOW)📦 Backing up InfluxDB data...$(NC)" && \
	if docker volume ls | grep -q "docker_influxdb_data"; then \
		docker run --rm -v docker_influxdb_data:/source -v "$$(pwd)/$(BACKUP_DIR)/$$BACKUP_NAME":/backup alpine tar czf /backup/influxdb_data.tar.gz -C /source .; \
	fi && \
	echo "$(YELLOW)📦 Backing up Grafana data...$(NC)" && \
	if docker volume ls | grep -q "docker_grafana_data"; then \
		docker run --rm -v docker_grafana_data:/source -v "$$(pwd)/$(BACKUP_DIR)/$$BACKUP_NAME":/backup alpine tar czf /backup/grafana_data.tar.gz -C /source .; \
	fi && \
	echo "$(GREEN)✅ Backup created at $(BACKUP_DIR)/$$BACKUP_NAME$(NC)"

# Restore from backup (requires BACKUP_NAME parameter)
restore:
	@if [ -z "$(BACKUP_NAME)" ]; then \
		echo "$(RED)❌ Please specify BACKUP_NAME. Example: make restore BACKUP_NAME=raven_backup_20240101_120000$(NC)"; \
		exit 1; \
	fi
	@echo "$(BLUE)📥 Restoring from backup $(BACKUP_NAME)...$(NC)"
	@if [ ! -d "$(BACKUP_DIR)/$(BACKUP_NAME)" ]; then \
		echo "$(RED)❌ Backup directory $(BACKUP_DIR)/$(BACKUP_NAME) not found$(NC)"; \
		exit 1; \
	fi
	@echo "$(YELLOW)⏹️  Stopping services...$(NC)"
	@$(MAKE) stop
	@echo "$(YELLOW)📥 Restoring InfluxDB data...$(NC)"
	@if [ -f "$(BACKUP_DIR)/$(BACKUP_NAME)/influxdb_data.tar.gz" ]; then \
		docker run --rm -v docker_influxdb_data:/target -v "$$(pwd)/$(BACKUP_DIR)/$(BACKUP_NAME)":/backup alpine tar xzf /backup/influxdb_data.tar.gz -C /target; \
	fi
	@echo "$(YELLOW)📥 Restoring Grafana data...$(NC)"
	@if [ -f "$(BACKUP_DIR)/$(BACKUP_NAME)/grafana_data.tar.gz" ]; then \
		docker run --rm -v docker_grafana_data:/target -v "$$(pwd)/$(BACKUP_DIR)/$(BACKUP_NAME)":/backup alpine tar xzf /backup/grafana_data.tar.gz -C /target; \
	fi
	@echo "$(YELLOW)▶️  Starting services...$(NC)"
	@$(MAKE) start
	@echo "$(GREEN)✅ Restore completed$(NC)"

# Clean up containers and networks
clean:
	@echo "$(BLUE)🧹 Cleaning up containers and networks...$(NC)"
	docker-compose -f $(COMPOSE_FILE) down --remove-orphans
	docker system prune -f
	@echo "$(GREEN)✅ Cleanup completed$(NC)"

# Clean up everything including volumes
clean-all:
	@echo "$(RED)⚠️  This will remove ALL data including volumes!$(NC)"
	@read -p "Are you sure? (y/N): " confirm && [ "$$confirm" = "y" ] || exit 1
	@echo "$(BLUE)🧹 Cleaning up everything...$(NC)"
	docker-compose -f $(COMPOSE_FILE) down -v --remove-orphans
	docker system prune -af --volumes
	@echo "$(GREEN)✅ Complete cleanup finished$(NC)"

## Utilities

# Show service URLs
urls:
	@echo "$(BLUE)🌐 Service URLs:$(NC)"
	@echo ""
	@echo "$(GREEN)📊 Grafana Dashboard:$(NC)     http://localhost:3000"
	@echo "   $(YELLOW)Username: raven$(NC)"
	@echo "   $(YELLOW)Password: ravens_see_all$(NC)"
	@echo ""
	@echo "$(GREEN)📈 Prometheus:$(NC)             http://localhost:9091"
	@echo "$(GREEN)🔍 Jaeger Tracing:$(NC)         http://localhost:16686"
	@echo "$(GREEN)💾 InfluxDB UI:$(NC)             http://localhost:8086"
	@echo "$(GREEN)🐦 Raven gRPC Server:$(NC)      localhost:50051"
	@echo "$(GREEN)❤️  Health Check:$(NC)           http://localhost:8080/health"
	@echo "$(GREEN)📊 Metrics Endpoint:$(NC)       http://localhost:9090/metrics"

# Open shell in running container
shell:
	@echo "$(BLUE)🐚 Opening shell in Market Data Server container...$(NC)"
	docker exec -it raven-server /bin/bash

# Open InfluxDB shell
db-shell:
	@echo "$(BLUE)🐚 Opening InfluxDB shell...$(NC)"
	docker exec -it raven-influxdb influx

# Development environment setup
dev-setup:
	@echo "$(BLUE)🛠️  Setting up development environment...$(NC)"
	@echo "$(YELLOW)Installing Rust toolchain...$(NC)"
	@if ! command -v rustc >/dev/null 2>&1; then \
		curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y; \
		source ~/.cargo/env; \
	fi
	@echo "$(YELLOW)Installing required components...$(NC)"
	rustup component add clippy rustfmt
	@echo "$(YELLOW)Installing protobuf compiler...$(NC)"
	@if command -v apt-get >/dev/null 2>&1; then \
		sudo apt-get update && sudo apt-get install -y protobuf-compiler; \
	elif command -v brew >/dev/null 2>&1; then \
		brew install protobuf; \
	else \
		echo "$(RED)Please install protobuf compiler manually$(NC)"; \
	fi
	@echo "$(GREEN)✅ Development environment setup completed$(NC)"

# Quick development cycle
dev: fmt lint test
	@echo "$(GREEN)✅ Development cycle completed$(NC)"

# Production deployment with backup
prod-deploy: backup deploy
	@echo "$(GREEN)✅ Production deployment completed with backup$(NC)"
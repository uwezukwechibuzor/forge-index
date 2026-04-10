.PHONY: build build-release test test-integration fmt lint clean \
       docker-build docker-push dev prod down logs migrate codegen bench

build:
	cargo build --workspace

build-release:
	cargo build --workspace --release

test:
	cargo test --workspace

test-integration:
	cargo test --workspace -- --include-ignored

fmt:
	cargo fmt --all

lint:
	cargo clippy --workspace -- -D warnings

clean:
	cargo clean

docker-build:
	docker build -t forge-index:latest .

docker-push:
	docker push forge-index:latest

dev:
	docker-compose -f docker-compose.dev.yml up

prod:
	docker-compose up -d

down:
	docker-compose down

logs:
	docker-compose logs -f indexer

migrate:
	cargo run -p forge-index-cli -- migrate

codegen:
	@echo "Usage: make codegen ABI=path/to/ABI.json NAME=ContractName"
	cargo run -p forge-index-cli -- codegen --abi $(ABI) --name $(NAME)

bench:
	cargo bench --workspace

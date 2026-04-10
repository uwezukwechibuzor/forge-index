# Contributing to forge-index

Thank you for your interest in contributing to forge-index.

## Development Setup

1. **Rust toolchain**: Install via [rustup](https://rustup.rs/). Minimum version: 1.75.
2. **Postgres**: Required for integration tests. Either install locally or use Docker.
3. **Docker**: Required for testcontainers-based integration tests.

```bash
git clone https://github.com/example/forge-index.git
cd forge-index
cargo build --workspace
cargo test --workspace
```

## Code Style

- Run `cargo fmt --all` before committing.
- Run `cargo clippy --workspace -- -D warnings` and fix all warnings.
- Public items must have `///` doc comments.
- Prefer explicit error handling over `.unwrap()` in library code.

## Testing

```bash
# Unit tests (no Docker required)
cargo test --workspace

# Integration tests (requires Docker)
cargo test --workspace -- --include-ignored

# Benchmarks
cargo bench -p forge-index
```

## Pull Requests

1. Fork the repository and create a feature branch.
2. Write tests for new functionality.
3. Ensure `cargo test --workspace` passes.
4. Ensure `cargo clippy --workspace -- -D warnings` passes.
5. Ensure `cargo fmt --all --check` passes.
6. Submit a pull request with a clear description.

## Architecture

See [docs/architecture.md](docs/architecture.md) for a detailed overview of the codebase structure and design decisions.

## License

By contributing, you agree that your contributions will be licensed under the MIT License.

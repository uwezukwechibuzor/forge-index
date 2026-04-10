# Stage 1 — Chef (base image with cargo-chef)
FROM rust:1.83-slim AS chef
RUN cargo install cargo-chef
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /app

# Stage 2 — Planner (compute dependency recipe)
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3 — Builder (compile dependencies then source)
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release -p forge-index-cli

# Stage 4 — Runtime (minimal final image)
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/forge /usr/local/bin/forge

EXPOSE 42069

ENTRYPOINT ["forge"]
CMD ["start"]

FROM rust:1.88-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY src ./src
COPY migrations ./migrations
COPY templates ./templates

RUN cargo build --release --locked

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install --no-install-recommends -y ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/indexflow-core /app/indexflow-core
COPY --from=builder /app/migrations /app/migrations
COPY --from=builder /app/templates /app/templates
COPY static /app/static

RUN mkdir -p /app/data

ENV SERVER_HOST=0.0.0.0 \
    SERVER_PORT=8080 \
    DATABASE_URL=sqlite:/app/data/indexflow.db?mode=rwc \
    DRY_RUN=true

EXPOSE 8080
VOLUME ["/app/data"]

CMD ["/app/indexflow-core"]

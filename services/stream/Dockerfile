# Build context: services/stream

FROM rust:1-slim-bookworm AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
# Compiled into the binary: sqlx::migrate! and the seed registry.
COPY migrations ./migrations
COPY data ./data
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/target/release/arc-stablecoin-stream /usr/local/bin/arc-stablecoin-stream
USER nobody
ENV PORT=8080
EXPOSE 8080
CMD ["arc-stablecoin-stream"]

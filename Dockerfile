FROM rust:1.83-bullseye AS builder

WORKDIR /app

COPY . .

RUN cargo build --release

FROM debian:bullseye-slim

WORKDIR /app
LABEL org.opencontainers.image.source=https://github.com/AcrylicShrimp/file-indexer
LABEL org.opencontainers.image.description="file-indexer v0.2.0"
LABEL org.opencontainers.image.licenses=MIT

RUN apt-get update && apt-get install -y ca-certificates

COPY --from=builder /app/target/release/file-indexer .

EXPOSE 8000

CMD ["./file-indexer"]

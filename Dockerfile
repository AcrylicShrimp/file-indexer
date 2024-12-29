FROM rust:1.83-bullseye AS builder

WORKDIR /app

COPY . .

RUN cargo build --release

FROM debian:bullseye-slim

WORKDIR /app

COPY --from=builder /app/target/release/file-indexer .

EXPOSE 8000

CMD ["./file-indexer"]

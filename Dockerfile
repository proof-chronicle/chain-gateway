# Build stage unchanged
FROM rust:1.86-slim as builder
WORKDIR /app
COPY . .
RUN apt-get update \
 && apt-get install -y pkg-config libssl-dev protobuf-compiler \
 && cargo build --release

# Runtime stage: use Bookworm, which has glibc>=2.34
FROM debian:bookworm-slim

WORKDIR /app
COPY --from=builder /app/target/release/chain-gateway .

# your existing CMD
CMD ["./chain-gateway"]
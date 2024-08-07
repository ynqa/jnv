# Build stage
FROM rust:1.80.0-slim-bookworm as builder

WORKDIR /jnv
COPY . /jnv
RUN cargo build --release

# Final stage
FROM debian:bookworm-slim

COPY --from=builder /jnv/target/release/jnv /bin/jnv

ENTRYPOINT ["/bin/jnv"]

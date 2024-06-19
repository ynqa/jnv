## Build stage
FROM rust:1.79-alpine3.20 as builder

RUN rustup target add x86_64-unknown-linux-musl

RUN apk add musl-dev

WORKDIR /jnv

COPY . /jnv

RUN cargo build --target=x86_64-unknown-linux-musl --release

## Final image

FROM scratch

COPY --from=builder /jnv/target/x86_64-unknown-linux-musl/release/jnv /bin/jnv

ENTRYPOINT ["/bin/jnv"]

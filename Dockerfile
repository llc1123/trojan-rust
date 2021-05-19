FROM rustlang/rust:nightly-alpine AS builder
RUN apk add alpine-sdk perl openssl-dev
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo install --target x86_64-unknown-linux-musl --path .

FROM alpine:latest
RUN apk --no-cache add openssl
COPY --from=builder /usr/local/cargo/bin/trojan-rust /trojan-rust/
COPY config.toml /trojan-rust/example.toml
WORKDIR /trojan-rust
ENTRYPOINT [ "./trojan-rust" ]
CMD [ "-c", "./example.toml" ]
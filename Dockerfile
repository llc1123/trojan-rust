FROM rustlang/rust:nightly-alpine AS builder
RUN apk add alpine-sdk perl
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo install --path .

FROM alpine:latest
COPY --from=builder /usr/local/cargo/bin/trojan-rust /trojan-rust/
COPY config.toml /trojan-rust/example.toml
WORKDIR /trojan-rust
ENTRYPOINT [ "./trojan-rust", "-c" ]
CMD [ "./example.toml" ]
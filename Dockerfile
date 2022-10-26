FROM rustlang/rust:nightly-bullseye-slim AS builder
RUN apt-get update && apt-get install -y perl make && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo install --path .

FROM debian:bullseye-slim
COPY --from=builder /usr/local/cargo/bin/trojan-rust /trojan-rust/
COPY config.toml /trojan-rust/example.toml
WORKDIR /trojan-rust
ENTRYPOINT [ "./trojan-rust", "-c" ]
CMD [ "./example.toml" ]
# -----------------
# Cargo Build Stage
# -----------------

FROM rust:latest as cargo-build

COPY . .
RUN apt-get update
RUN apt-get install -y cmake
RUN apt-get install -y clang
RUN cargo vendor > .cargo/config

RUN cargo build --release

# -----------------
# Run Momento Proxy
# -----------------

FROM debian:stable-slim

WORKDIR /app

ENV MOMENTO_AUTHENTICATION=""
ENV CONFIG="momento_proxy.toml"

RUN mkdir config

COPY --from=cargo-build ./target/release/momento_proxy .
COPY --from=cargo-build ./config/momento_proxy.toml ./config

RUN chmod +x ./momento_proxy
CMD ./momento_proxy ./config/${CONFIG}
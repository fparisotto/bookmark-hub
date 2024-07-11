# builder
FROM rust:1.79-bookworm as builder

# new empty project
RUN USER=root cargo new --bin daemon
WORKDIR /daemon

# copy manifests
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

# cache dependencies
RUN cargo build --release
RUN rm src/*.rs

# copy source tree
COPY ./src ./src

# build for release
RUN rm ./target/release/deps/daemon*
RUN cargo build --release

# runner
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates wait-for-it && update-ca-certificates

COPY --from=builder /daemon/target/release/daemon .

CMD ["./daemon"]

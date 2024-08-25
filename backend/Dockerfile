# builder
FROM rust:1.80-bookworm as builder

# new empty project
RUN USER=root cargo new --bin backend
WORKDIR /backend

# copy manifests
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

# cache dependencies
RUN cargo build --release
RUN rm src/*.rs

# copy source tree
COPY ./src ./src
COPY ./schema ./schema

# build for release
RUN rm ./target/release/deps/backend*
RUN cargo build --release

# runner
FROM debian:bookworm-slim

RUN apt update && apt install -y --no-install-recommends ca-certificates wait-for-it && update-ca-certificates

COPY --from=builder /backend/target/release/backend .

EXPOSE 3000
EXPOSE 9090

CMD ["./backend"]

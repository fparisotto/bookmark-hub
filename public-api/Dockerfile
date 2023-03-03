# builder
FROM rust:1.66.0-bullseye as builder

# new empty project
RUN USER=root cargo new --bin public-api
WORKDIR /public-api

# copy manifests
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
COPY ./wait-for-it.sh ./wait-for-it.sh 

# cache dependencies
RUN cargo build --release
RUN rm src/*.rs

# copy source tree
COPY ./src ./src
COPY ./migrations ./migrations

# build for release
RUN rm ./target/release/deps/public_api*
RUN cargo build --release

# runner
FROM debian:bullseye-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && update-ca-certificates

COPY --from=builder /public-api/target/release/public-api .
COPY --from=builder /public-api/wait-for-it.sh .

RUN chmod +x wait-for-it.sh

EXPOSE 3000
EXPOSE 9090

CMD ["./public-api"]

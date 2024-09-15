# builder
FROM rust:1.80-bookworm as builder

ARG BACKEND_URL
ENV BACKEND_URL=$BACKEND_URL

RUN echo $BACKEND_URL

ENV TRUNK_VERSION=v0.20.3

# curl
RUN apt-get update \
  && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl

# trunk
RUN curl -sLO https://github.com/thedodd/trunk/releases/download/$TRUNK_VERSION/trunk-x86_64-unknown-linux-gnu.tar.gz \
  && tar zxvf trunk-x86_64-unknown-linux-gnu.tar.gz \
  && chmod +x trunk \
  && mv trunk /usr/local/bin/

# wasm32 target
RUN rustup target add wasm32-unknown-unknown

# new empty project
RUN USER=root cargo new --bin web-spa
WORKDIR /web-spa

# copy manifests
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
COPY ./Trunk.toml ./Trunk.toml

# cache dependencies
RUN cargo build --release
RUN rm src/*.rs

# copy source tree
COPY ./src ./src
COPY ./index.html ./index.html

# build for release
RUN cargo clean && trunk build --release

# nginx static server
FROM nginx:alpine

WORKDIR /app

COPY --from=builder /web-spa/dist/* .

COPY ./nginx.conf /etc/nginx/nginx.conf

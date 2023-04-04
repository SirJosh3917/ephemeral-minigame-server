# Rust doesn't have official nightly version
# This makes our code prone to breaking, but oh well.
FROM rustlang/rust:nightly-bullseye-slim AS build
WORKDIR /src

COPY Cargo.lock Cargo.toml ./
RUN mkdir src && echo 'fn main() {}' >src/main.rs
RUN cargo fetch
RUN cargo build
RUN rm -rf src

COPY . .
RUN cargo build

FROM debian:bullseye-slim
WORKDIR /app
COPY --from=build /src/target/debug/controller /app/controller
ENTRYPOINT /app/controller

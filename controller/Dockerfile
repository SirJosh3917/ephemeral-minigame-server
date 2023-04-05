# Rust doesn't have official nightly version
# This makes our code prone to breaking, but oh well.
FROM rustlang/rust:nightly-bullseye-slim AS build
WORKDIR /src

# Not opting to have a separate dependency caching layer because there are some
# weird bugs with the ./target/debug/controller binary being the compiled artifact
# of the *old* stub main.rs... weird stuff man
COPY Cargo.lock Cargo.toml ./
RUN mkdir src && echo 'fn main() {println!("weird artifact bug wtf");}' >src/main.rs
RUN cargo fetch
RUN rm -rf ./src/

COPY . .
RUN cargo build

FROM debian:bullseye-slim
WORKDIR /app
COPY --from=build /src/target/debug/controller /app/controller
ENTRYPOINT /app/controller

FROM rust:latest

RUN apt-get update && apt-get install -y libopenblas-dev && rm -rf /var/lib/apt/lists/*
RUN rustup target add wasm32-unknown-unknown

WORKDIR /app

# indicators crate — copied to match the relative path ../rust/indicators from /app
COPY rust/indicators/ /rust/indicators/

# Build the builder binary
COPY builder/Cargo.toml Cargo.toml
COPY builder/build.rs build.rs
COPY builder/src/ src/

RUN cargo build --release

CMD ["./target/release/builder"]

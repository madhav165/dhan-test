FROM rust:latest

RUN rustup target add wasm32-unknown-unknown

WORKDIR /app

# indicators crate available at /app/indicators for strategy compilation
COPY rust/indicators/ indicators/

# Build the builder binary
COPY builder/Cargo.toml Cargo.toml
COPY builder/src/ src/

RUN cargo build --release

CMD ["./target/release/builder"]

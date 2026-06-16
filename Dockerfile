# Use the official Rust image as a builder
FROM rust:1.75-slim as builder

WORKDIR /app

# Install dependencies needed for compilation
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy the source code
COPY . .

# Build the application
RUN cargo build --release

# Final stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y libssl3 ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binary from the builder
COPY --from=builder /app/target/release/rust-websocket-service .

# Expose the port
EXPOSE 3000

# Run the application
CMD ["./rust-websocket-service"]

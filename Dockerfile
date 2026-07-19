# Build Stage
FROM rust:1.71-slim AS builder

WORKDIR /usr/src/fas

# Copy the application source code
COPY . .

# Compile the application in release mode
RUN cargo build --release

# Final runtime stage using secure and lightweight Distroless image
FROM gcr.io/distroless/cc-debian12

WORKDIR /app

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/fas/target/release/fas /usr/local/bin/fas

# Expose default application port
EXPOSE 8080

# Run the binary
ENTRYPOINT ["/usr/local/bin/fas"]

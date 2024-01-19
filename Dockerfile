# Stage 1: Build the application

FROM rust:1.71 as builder

ARG PROFILE=release
ARG BUILD_FEATURES
WORKDIR /app

# Update system packages and install build dependencies
RUN apt update -y && \
    apt install -y \
    cmake \
    pkg-config \
    libssl-dev \
    git \
    gcc \
    build-essential \
    clang \
    libclang-dev \
    protobuf-compiler \
    jq \
    libpq-dev

# Install rust wasm. Needed for substrate wasm engine
RUN rustup target add wasm32-unknown-unknown

# Copy the project files
COPY . .

# Build the application
RUN cargo build --features "$BUILD_FEATURES" --locked "--$PROFILE"

#Stage 2: Create the final image
FROM ubuntu:20.04

# Set the working directory
WORKDIR /app

# Copy the built binary from the builder stage
COPY --from=builder /app/target/release/lib* /app/target/release/vitreus-power-plant-node /app/target/release/
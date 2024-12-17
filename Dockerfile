# Stage 1: Build the application

FROM rust:1.83 AS builder

ARG PROFILE=release
ARG BUILD_FEATURES
WORKDIR /app

# Update system packages and install build dependencies
RUN apt update -y && \
    apt install -y \
    clang \
    protobuf-compiler

# Copy the project files
COPY . .

# Build the application
RUN cargo build --features "$BUILD_FEATURES" --locked "--$PROFILE"

#Stage 2: Create the final image
FROM ubuntu:24.04

# Set the working directory
WORKDIR /app

# Copy the built binary from the builder stage
COPY --from=builder \
    /app/chain-specs/* \
    /app/scripts/purge_chain.sh \
    /app/scripts/revert_chain.sh \
    /app/target/release/vitreus-power-plant-node \
    /app/target/release/polkadot-execute-worker \
    /app/target/release/polkadot-prepare-worker \
    /app/target/release/
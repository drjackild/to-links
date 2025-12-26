#!/bin/bash
set -e

# Configuration
RPI_HOST="rpi-b"
RPI_USER="drjackild"
TARGET_DIR="/home/$RPI_USER/to-links"
BINARY_NAME="app" # Based on Cargo.toml package name
TARGET_ARCH="aarch64-unknown-linux-gnu"

echo "Starting deployment for user $RPI_USER to $RPI_HOST..."

# 1. Build for aarch64
echo "Building binary for $TARGET_ARCH..."
if command -v cross &> /dev/null; then
    cross build --release --target $TARGET_ARCH
else
    # Ensure the linker is available or configured in .cargo/config.toml
    cargo build --release --target $TARGET_ARCH
fi

# 2. Deploy
echo "Copying binary to $RPI_HOST:$TARGET_DIR..."
# Ensure directory exists on RPi
ssh "$RPI_USER@$RPI_HOST" "mkdir -p $TARGET_DIR"
# Copy binary
scp "target/$TARGET_ARCH/release/$BINARY_NAME" "$RPI_USER@$RPI_HOST:$TARGET_DIR/to-links-app"

# 3. Restart Service
echo "Restarting to-links service on $RPI_HOST..."
# Using ssh -t for potential sudo interaction, though keys should handle auth
ssh -t "$RPI_USER@$RPI_HOST" "sudo systemctl restart to-links || echo 'Service not found or failed to restart (ignore if first deploy)'"

echo "Deployment successful!"
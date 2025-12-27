#!/bin/bash
set -e

# Load configuration
if [ -f .env ]; then
  # Use 'allexport' to correctly handle variables with spaces or special characters
  set -a
  source .env
  set +a
else
  echo "Error: .env file not found. Copy .env.example to .env and configure it."
  exit 1
fi

BINARY_NAME="app"

echo "Starting deployment to $RPI_HOST..."

# 1. Build
echo "Building binary for $TARGET_ARCH..."
if command -v cross &>/dev/null; then
  cross build --release --target $TARGET_ARCH --manifest-path app/Cargo.toml
else
  cargo build --release --target $TARGET_ARCH --manifest-path app/Cargo.toml
fi

# 2. Prepare Config Files
echo "Generating configuration files from templates..."
# Handle optional DB_PATH
if [ -n "$DB_PATH" ]; then
  export DB_ARG="--db $DB_PATH"
else
  export DB_ARG=""
fi

# Note: envsubst is part of gettext package
envsubst < systemd/to-links.service.template > systemd/to-links.service
envsubst < nginx/to-links.conf.template > nginx/to-links.conf
envsubst < dnsmasq/shortcuts.conf.template > dnsmasq/shortcuts.conf

# 3. Deploy
echo "Deploying to $RPI_HOST..."

# Stop service
ssh -t "$RPI_USER@$RPI_HOST" "sudo systemctl stop to-links || true"

# Create target directory
ssh "$RPI_USER@$RPI_HOST" "mkdir -p $TARGET_DIR"

# Copy binary
echo "Copying binary..."
scp "app/target/$TARGET_ARCH/release/$BINARY_NAME" "$RPI_USER@$RPI_HOST:$TARGET_DIR/to-links-app"

# Copy systemd service
echo "Installing systemd service..."
scp "systemd/to-links.service" "$RPI_USER@$RPI_HOST:/tmp/to-links.service"
ssh -t "$RPI_USER@$RPI_HOST" "sudo mv /tmp/to-links.service /etc/systemd/system/to-links.service && sudo systemctl daemon-reload"

# Copy generated configs to the target dir for convenience
scp "nginx/to-links.conf" "$RPI_USER@$RPI_HOST:$TARGET_DIR/"
scp "dnsmasq/shortcuts.conf" "$RPI_USER@$RPI_HOST:$TARGET_DIR/"

# 4. Start Service
echo "Starting service..."
ssh -t "$RPI_USER@$RPI_HOST" "sudo systemctl enable to-links && sudo systemctl start to-links"

echo "Deployment successful!"
echo "NOTE: Generated Nginx and Dnsmasq configs are at $TARGET_DIR on the remote host."
# To-Links Application

This is the URL shortener application component.

## Deployment on Raspberry Pi (aarch64)

### Prerequisites

1.  **Cross-Compilation Target**:
    ```bash
    rustup target add aarch64-unknown-linux-gnu
    ```
2.  **Linker**: Install `aarch64-linux-gnu-gcc` on your host system.
    [Optional] you can install `cross` to make it easier:
    ```bash
    cargo install cross
    ```
3.  **SSH**: SSH public key authentication should be configured for user `drjackild` on `rpi-b`.

### Deployment Script

Use the provided `deploy.sh` script to build and upload the binary:

```bash
chmod +x deploy.sh
./deploy.sh
```

### Systemd Service Configuration

On the Raspberry Pi, create the service file at `/etc/systemd/system/to-links.service`:

```ini
[Unit]
Description=To-Links Shortener Service
After=network.target

[Service]
Type=simple
User=drjackild
WorkingDirectory=/home/drjackild/to-links
# Starts the app and points to the database file in the same directory
ExecStart=/home/drjackild/to-links/to-links-app --db /home/drjackild/to-links/app.db
Restart=always
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
```

### Management Commands

Once the service file is created, run these commands on the RPi:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now to-links
sudo systemctl status to-links
```

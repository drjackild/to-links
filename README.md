# Raspberry Pi Shortcut Network

This project provides a local DNS-based shortcut system for your home network.
- `http://maps/` and `http://excalidraw/` are static Nginx redirects (as an examples, you can add/delete as you wish)
- `http://to/` handles dynamic user-defined links stored in SQLite.
- `http://to/link` provides a management UI powered by Rust & HTMX.

## DNS Server Configuration (dnsmasq)
Ensure `/etc/dnsmasq.conf` on your Raspberry Pi includes:
```text
expand-hosts
domain=lan
local=/lan/
```

## Local Hosts Entry
Add your Raspberry Pi's IP and the hostnames to /etc/hosts:

```text
192.168.1.78  to maps excalidraw
```

## Nginx Configuration
Copy `./nginx/to-links.conf` to `/etc/nginx/sites-available/`

Enable it and restart Nginx:

```bash
sudo ln -s /etc/nginx/sites-available/to-links.conf /etc/nginx/sites-enabled/
sudo nginx -t && sudo systemctl reload nginx
```

## Rust Application Setup

Build the binary: `cargo build --release`

Run the application: `./target/release/to-link-app` Note: The app listens on 127.0.0.1:3000 by default.

## Client Machine Setup (Windows/Mac)
Ensure your machine's DNS is pointing to the Raspberry Pi IP.

## Systemd Service Configuration

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


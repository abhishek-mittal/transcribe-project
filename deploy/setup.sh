#!/usr/bin/env bash
# Deploy the transcribe-project on a freshly provisioned Vultr VPS.
# Run as root after `terraform apply` has finished and cloud-init completed.
#
# Usage:
#   REPO_URL=https://github.com/abhishek-mittal/shuhari.git \
#     [WHISPER_MODEL=tiny] [DOMAIN=transcribe.example.com] \
#     bash deploy/setup.sh

set -euo pipefail

APP_DIR="/opt/transcribe"
REPO_URL="${REPO_URL:-}"
WHISPER_MODEL="${WHISPER_MODEL:-tiny}"
DOMAIN="${DOMAIN:-}"

if [ -z "$REPO_URL" ]; then
  echo "ERROR: set REPO_URL env var before running." >&2
  exit 1
fi

echo "==> [0/7] Ensuring Node.js 20+ is installed..."
NODE_MAJOR="$(node --version 2>/dev/null | sed -E 's/^v([0-9]+).*/\1/' || echo 0)"
if [ "${NODE_MAJOR:-0}" -lt 20 ]; then
  echo "    Installing Node.js 20 from NodeSource (current: ${NODE_MAJOR:-none})..."
  apt-get remove -y -qq nodejs npm libnode-dev libnode72 >/dev/null 2>&1 || true
  curl -fsSL https://deb.nodesource.com/setup_20.x | bash -
  apt-get install -y -qq nodejs
fi
echo "    node $(node --version) / npm $(npm --version)"

echo "==> [1/7] Cloning / updating repository..."
if [ -d "$APP_DIR/.git" ]; then
  git -C "$APP_DIR" fetch --quiet origin
  git -C "$APP_DIR" reset --hard origin/main
else
  git clone "$REPO_URL" "$APP_DIR"
fi

cd "$APP_DIR"

echo "==> [2/7] Setting up Python environment..."
python3 -m venv .venv
.venv/bin/pip install --quiet --upgrade pip
.venv/bin/pip install --quiet -r requirements.txt

echo "    Installing bgutil PO-token Node helper (global)..."
npm install -g --silent bgutil-ytdlp-pot-provider 2>/dev/null || \
  echo "    (warning: bgutil node helper install failed; PO tokens disabled)"

echo "==> [3/7] Pre-downloading Whisper model: $WHISPER_MODEL"
WHISPER_MODEL="$WHISPER_MODEL" .venv/bin/python scripts/predownload_model.py

echo "==> [4/7] Building SvelteKit frontend..."
npm ci --silent
npm run build

echo "==> [5/7] Setting file ownership..."
chown -R www-data:www-data "$APP_DIR"

echo "==> [6/7] Installing systemd services..."
cp deploy/transcribe-api.service /etc/systemd/system/
cp deploy/sveltekit.service /etc/systemd/system/
systemctl daemon-reload
systemctl enable --now transcribe-api sveltekit

echo "==> [7/7] Configuring Nginx..."
cp deploy/nginx.conf /etc/nginx/sites-available/transcribe
ln -sf /etc/nginx/sites-available/transcribe /etc/nginx/sites-enabled/transcribe
rm -f /etc/nginx/sites-enabled/default
nginx -t && systemctl reload nginx

if [ -n "$DOMAIN" ]; then
  echo "==> Requesting TLS certificate for $DOMAIN..."
  certbot --nginx -d "$DOMAIN" --non-interactive --agree-tos -m "admin@$DOMAIN"
fi

echo ""
echo "==> Deployment complete!"
PUBLIC_IP="$(curl -s ifconfig.me || echo unknown)"
echo "==> App: http://${PUBLIC_IP}"
[ -n "$DOMAIN" ] && echo "==> App: https://${DOMAIN}"

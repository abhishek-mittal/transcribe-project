#!/usr/bin/env bash
# Deploy the transcribe-project on a freshly provisioned Vultr VPS.
# Run as root after `terraform apply` has finished and cloud-init completed.
#
# Usage:
#   REPO_URL=https://github.com/abhishek-mittal/shuhari.git \
#     [WHISPER_MODEL=tiny] \
#     [DOMAIN=transcribe.example.com] \
#     [YT_DLP_PROXY=socks5://127.0.0.1:40000] \
#     [YT_DLP_PLAYER_CLIENTS=android] \
#     bash deploy/setup.sh
#
# Steps:
#   0  Node.js 20
#   1  Clone / update repo
#   2  Python venv + pip
#   3  Whisper model pre-download
#   4  SvelteKit build
#   5  File ownership
#   6  systemd services
#   7  Nginx (HTTP-only, or HTTPS when $DOMAIN is set)
#   +  Cloudflare WARP proxy (for YouTube bot-bypass)
#   +  /etc/transcribe/api.env (yt-dlp env vars)

set -euo pipefail

APP_DIR="/opt/transcribe"
REPO_URL="${REPO_URL:-}"
WHISPER_MODEL="${WHISPER_MODEL:-tiny}"
DOMAIN="${DOMAIN:-}"
# yt-dlp bot-bypass defaults — override via env before running
YT_DLP_PROXY="${YT_DLP_PROXY:-socks5://127.0.0.1:40000}"
YT_DLP_PLAYER_CLIENTS="${YT_DLP_PLAYER_CLIENTS:-android}"

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
ln -sf /etc/nginx/sites-available/transcribe /etc/nginx/sites-enabled/transcribe
rm -f /etc/nginx/sites-enabled/default

if [ -n "$DOMAIN" ]; then
  # Step 1: install HTTP-only config so certbot can serve the ACME challenge
  cp deploy/nginx.conf /etc/nginx/sites-available/transcribe
  nginx -t && systemctl reload nginx

  # Step 2: obtain Let's Encrypt certificate (certonly = get cert, don't modify nginx)
  echo "    Obtaining Let's Encrypt certificate for $DOMAIN..."
  certbot certonly --nginx -d "$DOMAIN" --non-interactive --agree-tos -m "admin@$DOMAIN"

  # Step 3: install the full SSL config with our SSE buffering settings
  echo "    Installing SSL nginx config..."
  sed "s/__DOMAIN__/$DOMAIN/g" deploy/nginx-ssl.conf > /etc/nginx/sites-available/transcribe
  nginx -t && systemctl reload nginx
else
  cp deploy/nginx.conf /etc/nginx/sites-available/transcribe
  nginx -t && systemctl reload nginx
fi

echo "==> Installing Cloudflare WARP (yt-dlp proxy)..."
if ! command -v warp-cli >/dev/null 2>&1; then
  curl -fsSL https://pkg.cloudflareclient.com/pubkey.gpg \
    | gpg --yes --dearmor -o /usr/share/keyrings/cloudflare-warp-archive-keyring.gpg
  echo "deb [signed-by=/usr/share/keyrings/cloudflare-warp-archive-keyring.gpg] \
https://pkg.cloudflareclient.com/ $(lsb_release -cs) main" \
    > /etc/apt/sources.list.d/cloudflare-client.list
  apt-get update -qq
  apt-get install -y -qq cloudflare-warp
fi
systemctl enable --now warp-svc
# Register (idempotent — fails silently if already registered)
warp-cli registration new --accept-tos 2>/dev/null || true
warp-cli mode proxy 2>/dev/null || true
warp-cli proxy port 40000 2>/dev/null || true
warp-cli connect 2>/dev/null || true
sleep 3
warp-cli status || true

echo "==> Creating /etc/transcribe/api.env..."
mkdir -p /etc/transcribe/cookies
# Preserve existing api.env on re-runs (may contain custom values or cookie paths)
if [ ! -f /etc/transcribe/api.env ]; then
  cat > /etc/transcribe/api.env << EOF
YT_DLP_PROXY=${YT_DLP_PROXY}
YT_DLP_PLAYER_CLIENTS=${YT_DLP_PLAYER_CLIENTS}
YT_DLP_COOKIES_FILE=/etc/transcribe/cookies/youtube.txt
IG_DLP_COOKIES_FILE=/etc/transcribe/cookies/instagram.txt
EOF
  chmod 640 /etc/transcribe/api.env
  echo "    Created /etc/transcribe/api.env"
else
  echo "    /etc/transcribe/api.env already exists — skipping (preserving existing config)"
fi

echo ""
echo "==> Deployment complete!"
PUBLIC_IP="$(curl -s ifconfig.me || echo unknown)"
echo "==> App: http://${PUBLIC_IP}"
[ -n "$DOMAIN" ] && echo "==> App: https://${DOMAIN}"

#!/usr/bin/env bash
# Idempotent redeploy on an already-provisioned Vultr VPS.
# Invoked by .github/workflows/deploy.yml over SSH on every push to main.
#
# Assumes deploy/setup.sh has been run once on this host.

set -euo pipefail

APP_DIR="${APP_DIR:-/opt/transcribe}"

cd "$APP_DIR"

git config --global --add safe.directory "$APP_DIR"

echo "==> git pull"
git fetch --quiet origin
git reset --hard origin/main

echo "==> pip install"
.venv/bin/pip install --quiet --upgrade pip
.venv/bin/pip install --quiet -r requirements.txt

echo "==> ensure bgutil PO-token Node helper"
if ! command -v bgutil-pot-provider >/dev/null 2>&1; then
  npm install -g --silent bgutil-ytdlp-pot-provider 2>/dev/null || \
    echo "  (warning: bgutil node helper install failed; PO tokens disabled)"
fi

echo "==> predownload model"
.venv/bin/python scripts/predownload_model.py

echo "==> ensure Node.js 20"
node_major="$(node -p 'process.versions.node.split(".")[0]' 2>/dev/null || echo 0)"
if [ "$node_major" -lt 20 ]; then
  echo "  installing Node.js 20 (current: $(node --version 2>/dev/null || echo none))"
  apt-get remove -y -qq nodejs npm libnode-dev libnode72 2>/dev/null || true
  apt-get autoremove -y -qq 2>/dev/null || true
  curl -fsSL https://deb.nodesource.com/setup_20.x | bash -
  apt-get install -y -qq nodejs
fi

echo "==> npm ci + build"
npm ci --silent
npm run build

echo "==> chown"
chown -R www-data:www-data "$APP_DIR"

echo "==> restart services"
systemctl daemon-reload
systemctl restart transcribe-api sveltekit
systemctl --no-pager --lines=0 status transcribe-api sveltekit

echo "==> redeploy complete"

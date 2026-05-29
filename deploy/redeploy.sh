#!/usr/bin/env bash
# Idempotent redeploy on an already-provisioned Vultr VPS.
# Invoked by .github/workflows/deploy.yml over SSH on every push to main.
#
# Assumes deploy/setup.sh has been run once on this host.

set -euo pipefail

APP_DIR="${APP_DIR:-/opt/transcribe}"

cd "$APP_DIR"

echo "==> git pull"
git fetch --quiet origin
git reset --hard origin/main

echo "==> pip install"
.venv/bin/pip install --quiet --upgrade pip
.venv/bin/pip install --quiet -r requirements.txt

echo "==> predownload model"
.venv/bin/python scripts/predownload_model.py

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

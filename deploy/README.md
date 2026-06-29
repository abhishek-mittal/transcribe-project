# Deploy (Legacy / Reference)

> **Note**: The primary product is now the Tauri desktop app (`src-tauri/`).
> This directory contains the **dormant** Vultr + Nginx + systemd + Terraform
> deployment that ran the Flask server. It is preserved for:
>
> - Reference when porting the sidecar logic back to a hosted offering
> - Reproducing the previous production environment for testing
> - A potential v2.0 hosted path (e.g., accounts, cloud sync)
>
> Do not deploy from this directory without first checking that the Flask
> server is still functional:
>
> ```bash
> source .venv/bin/activate
> python scripts/dev_api.py   # local sanity check
> ```

## Contents

- `setup.sh` — initial VPS setup (deps, model download, systemd units)
- `redeploy.sh` — re-pull repo + restart services
- `destroy.sh` — terraform destroy + cleanup
- `nginx.conf` / `nginx-ssl.conf` — reverse proxy + TLS config
- `sveltekit.service` — systemd unit for the SvelteKit frontend (Node SSR)
- `transcribe-api.service` — systemd unit for the Flask API (Gunicorn)
- `terraform/` — Vultr VPS provisioning (main.tf, variables.tf, outputs.tf)

## Why this is dormant

The hosted path was abandoned because datacenter-IP blocking from YouTube
and Instagram made the core transcription flow unreliable. The Tauri
desktop app runs yt-dlp from the user's residential IP, eliminating the
problem at the source.

See [README.md](../README.md) for the active desktop app workflow.
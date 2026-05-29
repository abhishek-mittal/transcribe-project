---
name: vultr-hosting
description: 'Deploy and host the transcribe-project on Vultr. Use for: migrate from Vercel to Vultr, deploy SvelteKit frontend, deploy Python transcription API, configure Vultr App Platform, Vultr Cloud Functions, or VPS, set up environment variables, configure custom domains, troubleshoot Vultr deployments.'
argument-hint: 'e.g. deploy frontend, deploy API, configure VPS, set up domain'
---

# Vultr Hosting — transcribe-project

## Overview

This project has two deployable units:

| Unit | Tech | Vultr Target |
|------|------|-------------|
| Frontend | SvelteKit + TypeScript | App Platform (Node.js) or VPS |
| Transcription API | Python + faster-whisper + yt-dlp | VPS (recommended) or Cloud Functions |

> **Note**: The Python API bundles a Whisper model (~40–150 MB) and runs `yt-dlp`. Vultr Cloud Functions has a 50 MB package limit — use a **VPS** or **Bare Metal** instance for the API unless you use a remote model download approach.

---

## Option A — VPS (Full Control, Recommended)

Best for this project because the Whisper model and yt-dlp require more disk/memory than serverless allows.

### 1. Provision a VPS

- Minimum: **2 vCPU, 4 GB RAM** (for `tiny`/`base` model)
- Recommended: **4 vCPU, 8 GB RAM** (for `small`/`medium` model)
- OS: Ubuntu 22.04 LTS
- Region: closest to your users

```bash
# After SSH into the VPS:
sudo apt update && sudo apt upgrade -y
sudo apt install -y python3-pip python3-venv nodejs npm ffmpeg git
```

### 2. Deploy the Python API

```bash
git clone <your-repo> /opt/transcribe
cd /opt/transcribe
python3 -m venv .venv
.venv/bin/pip install -r requirements.txt

# Pre-download the Whisper model
WHISPER_MODEL=tiny .venv/bin/python scripts/predownload_model.py
```

Run with **Gunicorn** behind **Nginx**:

```bash
pip install gunicorn
# api/transcribe.py exports a WSGI/ASGI app — adapt as needed
gunicorn -w 2 -b 127.0.0.1:8000 api.transcribe:app
```

Set up a systemd service at `/etc/systemd/system/transcribe-api.service`:

```ini
[Unit]
Description=Transcribe API
After=network.target

[Service]
User=www-data
WorkingDirectory=/opt/transcribe
ExecStart=/opt/transcribe/.venv/bin/gunicorn -w 2 -b 127.0.0.1:8000 api.transcribe:app
Restart=always
Environment=WHISPER_MODEL=tiny

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl enable --now transcribe-api
```

### 3. Deploy the SvelteKit Frontend

```bash
cd /opt/transcribe
npm install
npm run build            # outputs to .svelte-kit/output or build/
```

Serve with Nginx (static adapter) or Node.js adapter:

```nginx
server {
    listen 80;
    server_name your-domain.com;

    # Static files (if using @sveltejs/adapter-static)
    root /opt/transcribe/build;
    try_files $uri $uri/ /index.html;

    # Proxy API calls to Python backend
    location /api/ {
        proxy_pass http://127.0.0.1:8000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_read_timeout 120s;   # transcription can be slow
    }
}
```

### 4. Environment Variables

Set in `/etc/systemd/system/transcribe-api.service` under `[Service]`:

```ini
Environment=WHISPER_MODEL=tiny
Environment=MAX_DURATION=60
```

Or use a `.env` file loaded by the app.

---

## Option B — Vultr App Platform

Good for the **frontend only**. The Python API still needs a VPS due to model size.

### Deploy Frontend to App Platform

1. Go to [Vultr App Platform](https://my.vultr.com/apps/)
2. Connect your GitHub repository
3. Set:
   - **Build command**: `npm run build`
   - **Output directory**: `build` (or `.svelte-kit/output/client` for `adapter-node`)
   - **Runtime**: Node.js 20
4. Add environment variable: `PUBLIC_API_URL=https://your-api-vps-ip-or-domain/api`

> Update `src/routes/+page.svelte` to use `PUBLIC_API_URL` for API calls when not on Vercel.

---

## Option C — Vultr Object Storage (Static Frontend)

Use only if the SvelteKit app is fully static (`adapter-static`).

```bash
# Install s3cmd or aws-cli (Vultr Object Storage is S3-compatible)
s3cmd put --recursive build/ s3://your-bucket/ --acl-public
```

Configure CORS and set the index/error documents in the Vultr console.

---

## Switching from Vercel

Key changes needed when moving off Vercel:

| Concern | Vercel | Vultr VPS |
|---------|--------|-----------|
| Serverless handler | `vercel.json` + `handler(req,res)` | Gunicorn / FastAPI / Flask app |
| Build command | `vercel.json → buildCommand` | Makefile / deploy script |
| Function timeout | `maxDuration: 60` | Nginx `proxy_read_timeout` |
| Model files | `includeFiles: api/_models/**` | Pre-downloaded on disk |
| HTTPS/TLS | Automatic | Certbot: `certbot --nginx -d your-domain.com` |

---

## Procedure: Full VPS Deploy

1. Provision VPS (Ubuntu 22.04, 4 GB RAM)
2. SSH in, install deps (`ffmpeg`, `python3`, `node`, `npm`, `nginx`)
3. Clone repo to `/opt/transcribe`
4. `pip install -r requirements.txt`
5. `WHISPER_MODEL=tiny python scripts/predownload_model.py`
6. Set up systemd service for the API
7. `npm install && npm run build` for the frontend
8. Configure Nginx with proxy to API
9. `certbot --nginx` for HTTPS
10. Point DNS A record to VPS IP

---

## References

- [Vultr Docs — Deploy a Node.js App](https://docs.vultr.com/how-to-deploy-a-node-js-application)
- [Vultr App Platform Docs](https://docs.vultr.com/vultr-app-platform)
- [Certbot Nginx Guide](https://certbot.eff.org/instructions?os=ubuntufocal&webserver=nginx)
- [faster-whisper repo](https://github.com/SYSTRAN/faster-whisper)
- [yt-dlp installation](https://github.com/yt-dlp/yt-dlp#installation)

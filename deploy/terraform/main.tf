terraform {
  required_version = ">= 1.6"

  required_providers {
    vultr = {
      source  = "vultr/vultr"
      version = "~> 2.22"
    }
  }
}

provider "vultr" {
  api_key = var.vultr_api_key
}

# ── OS ──────────────────────────────────────────────────────────────────────
data "vultr_os" "ubuntu" {
  filter {
    name   = "name"
    values = ["Ubuntu 22.04 LTS x64"]
  }
}

# ── Firewall ────────────────────────────────────────────────────────────────
resource "vultr_firewall_group" "transcribe" {
  description = "transcribe-project"
}

resource "vultr_firewall_rule" "ssh" {
  firewall_group_id = vultr_firewall_group.transcribe.id
  protocol          = "tcp"
  ip_type           = "v4"
  subnet            = "0.0.0.0"
  subnet_size       = 0
  port              = "22"
  notes             = "SSH"
}

resource "vultr_firewall_rule" "http" {
  firewall_group_id = vultr_firewall_group.transcribe.id
  protocol          = "tcp"
  ip_type           = "v4"
  subnet            = "0.0.0.0"
  subnet_size       = 0
  port              = "80"
  notes             = "HTTP"
}

resource "vultr_firewall_rule" "https" {
  firewall_group_id = vultr_firewall_group.transcribe.id
  protocol          = "tcp"
  ip_type           = "v4"
  subnet            = "0.0.0.0"
  subnet_size       = 0
  port              = "443"
  notes             = "HTTPS"
}

# ── VPS ─────────────────────────────────────────────────────────────────────
resource "vultr_instance" "transcribe" {
  plan              = var.plan
  region            = var.region
  os_id             = data.vultr_os.ubuntu.id
  label             = "transcribe-project"
  hostname          = "transcribe"
  firewall_group_id = vultr_firewall_group.transcribe.id
  ssh_key_ids       = var.ssh_key_ids
  backups           = "disabled"
  enable_ipv6       = false

  # Bootstrap: install system dependencies on first boot via cloud-init.
  # Application deploy (clone repo, pip install, npm build, systemd units) is
  # handled separately by deploy/setup.sh after the instance is reachable.
  user_data = <<-EOT
    #!/bin/bash
    set -e
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    apt-get install -y -qq \
      git curl ffmpeg \
      python3-pip python3-venv \
      nginx \
      certbot python3-certbot-nginx
    # Node.js 20 from NodeSource (Ubuntu 22.04 ships Node 12 which is too old for Vite)
    curl -fsSL https://deb.nodesource.com/setup_20.x | bash -
    apt-get install -y -qq nodejs
    echo "WHISPER_MODEL=${var.whisper_model}" >> /etc/environment
    echo "LOG_LEVEL=INFO" >> /etc/environment
  EOT
}

variable "vultr_api_key" {
  description = "Vultr API key. Set via TF_VAR_vultr_api_key env var (do not commit)."
  type        = string
  sensitive   = true
}

variable "region" {
  description = "Vultr region slug. ewr=New Jersey, lax=Los Angeles, fra=Frankfurt, sin=Singapore, blr=Bangalore."
  type        = string
  default     = "ewr"
}

variable "plan" {
  description = "Vultr plan. vc2-1c-2gb = $12/mo (recommended). vc2-1c-1gb = $6/mo (tight for tiny model)."
  type        = string
  default     = "vc2-1c-2gb"
}

variable "ssh_key_ids" {
  description = "Vultr SSH key IDs to authorize on the instance. Find IDs in Vultr console > Account > SSH Keys."
  type        = list(string)
  default     = []
}

variable "repo_url" {
  description = "Git repository URL to clone on the server during setup.sh."
  type        = string
}

variable "whisper_model" {
  description = "Whisper model to pre-download. tiny (~75 MB) or base (~141 MB) for 2 GB VPS."
  type        = string
  default     = "tiny"

  validation {
    condition     = contains(["tiny", "base", "small"], var.whisper_model)
    error_message = "whisper_model must be 'tiny', 'base', or 'small'."
  }
}

variable "domain" {
  description = "Optional domain for Certbot TLS. Leave empty to use bare IP (HTTP only)."
  type        = string
  default     = ""
}

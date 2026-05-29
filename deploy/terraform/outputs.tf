output "instance_ip" {
  description = "Public IPv4 address of the VPS."
  value       = vultr_instance.transcribe.main_ip
}

output "ssh_command" {
  description = "SSH command to connect to the VPS."
  value       = "ssh root@${vultr_instance.transcribe.main_ip}"
}

output "setup_command" {
  description = "One-liner to deploy the app after SSH is available."
  value       = "ssh root@${vultr_instance.transcribe.main_ip} 'curl -fsSL https://raw.githubusercontent.com/<OWNER>/<REPO>/main/deploy/setup.sh | REPO_URL=${var.repo_url} WHISPER_MODEL=${var.whisper_model} DOMAIN=${var.domain} bash'"
}

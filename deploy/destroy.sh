#!/usr/bin/env bash
# Tear down every Vultr resource provisioned for transcribe-project so
# billing stops. Wraps `terraform destroy` (the safe, state-aware way to
# remove what main.tf created) rather than calling the Vultr API by hand.
#
# Resources destroyed: the vc2 instance (billed hourly/monthly) and its
# firewall group (free, but removed for a clean slate).
#
# Usage:
#   cd deploy/terraform && ../destroy.sh
#   or: bash deploy/destroy.sh   (run from anywhere; cd's into terraform/ itself)
#
# Flags:
#   --yes     skip the interactive confirmation prompt
#   --plan    only show what would be destroyed; don't actually destroy

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TF_DIR="$SCRIPT_DIR/terraform"

AUTO_YES=false
PLAN_ONLY=false
for arg in "$@"; do
  case "$arg" in
    --yes) AUTO_YES=true ;;
    --plan) PLAN_ONLY=true ;;
    *) echo "Unknown flag: $arg" >&2; exit 1 ;;
  esac
done

if [ ! -d "$TF_DIR" ]; then
  echo "ERROR: terraform dir not found at $TF_DIR" >&2
  exit 1
fi

cd "$TF_DIR"

if [ ! -f terraform.tfvars ] && [ -z "${TF_VAR_vultr_api_key:-}" ]; then
  echo "ERROR: no terraform.tfvars and TF_VAR_vultr_api_key is unset." >&2
  echo "       Set one of these so terraform can authenticate to Vultr." >&2
  exit 1
fi

echo "==> terraform init (no-op if already initialized)"
terraform init -input=false >/dev/null

echo "==> Resources currently tracked in state:"
terraform state list || echo "  (none — nothing to destroy)"
echo ""

echo "==> Plan: what 'destroy' would remove"
terraform plan -destroy -input=false

if [ "$PLAN_ONLY" = true ]; then
  echo ""
  echo "==> --plan passed, stopping before destroy."
  exit 0
fi

echo ""
if [ "$AUTO_YES" != true ]; then
  read -r -p "Type 'destroy' to permanently delete the resources above: " confirm
  if [ "$confirm" != "destroy" ]; then
    echo "Aborted — no changes made."
    exit 1
  fi
fi

echo "==> terraform destroy"
terraform destroy -auto-approve

echo ""
echo "==> Verifying nothing is left in state:"
remaining="$(terraform state list || true)"
if [ -z "$remaining" ]; then
  echo "  state is empty — all tracked resources destroyed."
else
  echo "  WARNING: resources still in state:"
  echo "$remaining"
  echo "  Re-run this script or investigate manually before assuming billing has stopped."
  exit 1
fi

echo ""
echo "==> Done. The Vultr VPS and its firewall group have been deleted."
echo "==> Double-check the Vultr console (https://my.vultr.com/) to confirm"
echo "    no instance/snapshot/backup remains, since anything created"
echo "    outside Terraform (manual snapshots, extra firewall rules) is"
echo "    not tracked in state and won't be removed by this script."

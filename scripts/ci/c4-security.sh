#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

mkdir -p artifacts/security
export GRYPE_DB_AUTO_UPDATE=false

if command -v trivy >/dev/null 2>&1; then
  trivy fs --offline-scan --skip-db-update --format json --output artifacts/security/trivy-fs.json . \
    || echo "WARNING: trivy scan failed (exit $?), continuing" >&2
else
  echo '{"status":"trivy-not-installed"}' > artifacts/security/trivy-fs.json
fi

if command -v syft >/dev/null 2>&1; then
  syft dir:. -o cyclonedx-json > artifacts/security/sbom.cdx.json \
    || echo "WARNING: syft CycloneDX generation failed (exit $?)" >&2
  syft dir:. -o spdx-json > artifacts/security/sbom.spdx.json \
    || echo "WARNING: syft SPDX generation failed (exit $?)" >&2
else
  echo '{"status":"syft-not-installed"}' > artifacts/security/sbom.cdx.json
  echo '{"status":"syft-not-installed"}' > artifacts/security/sbom.spdx.json
fi

if command -v grype >/dev/null 2>&1; then
  grype dir:. -o json > artifacts/security/grype.json \
    || echo "WARNING: grype scan failed (exit $?)" >&2
else
  echo '{"status":"grype-not-installed"}' > artifacts/security/grype.json
fi

if command -v gitleaks >/dev/null 2>&1; then
  gitleaks detect --source . --report-format json --report-path artifacts/security/gitleaks.json \
    || echo "WARNING: gitleaks scan failed (exit $?)" >&2
else
  echo '{"status":"gitleaks-not-installed"}' > artifacts/security/gitleaks.json
fi

if command -v oscap >/dev/null 2>&1; then
  oscap --version > artifacts/security/oscap.txt \
    || echo "WARNING: oscap version check failed" >&2
else
  echo 'oscap-not-installed' > artifacts/security/oscap.txt
fi

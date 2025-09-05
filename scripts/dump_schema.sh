#!/usr/bin/env bash
set -euo pipefail
if [[ -z "${DATABASE_URL:-}" ]]; then
  echo "DATABASE_URL no definido" >&2
  exit 1
fi
OUT="documentation/snapshots/schema_f3.sql"
pg_dump --schema-only --no-owner --no-privileges \
  -t event_log -t workflow_step_artifacts "$DATABASE_URL" > "$OUT.tmp"
# Normalizar (remover comentarios de pg_dump excepto nuestras marcas)
sed -i '/^-- Dumped/d' "$OUT.tmp"
mv "$OUT.tmp" "$OUT"
sha256sum "$OUT" | awk '{print $1}' > "${OUT}.sha256"
echo "Snapshot actualizado: $OUT"
echo "Hash: $(cat ${OUT}.sha256)"

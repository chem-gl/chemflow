#!/usr/bin/env bash
set -euo pipefail

# Ejecuta cargo metadata para obtener información del grafo de dependencias
# y falla tempranamente si hay problemas.
cargo metadata --format-version 1 > /dev/null

# Detecta ciclos de dependencias usando cargo tree
if cargo tree --edges normal | grep -q 'cycle'; then
  echo "Dependency cycle detected" >&2
  exit 1
else
  echo "No dependency cycles detected"
fi

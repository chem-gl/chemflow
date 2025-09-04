#!/usr/bin/env bash
set -euo pipefail

# Ejecuta Clippy en todos los targets y características, fallando en warnings
echo "Running Clippy..."
cargo clippy --all-targets --all-features -- -D warnings

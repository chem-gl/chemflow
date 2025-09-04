#!/usr/bin/env bash
set -euo pipefail

# Ejecuta la suite completa de tests
echo "Running all tests..."
cargo test --all 

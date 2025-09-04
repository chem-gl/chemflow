#!/usr/bin/env bash
set -euo pipefail

# Cargar solo las variables necesarias del .env
if [ -f ".env" ]; then
    export LD_LIBRARY_PATH=$(grep '^LD_LIBRARY_PATH=' .env | cut -d '=' -f2-)
    export PYO3_PYTHON=$(grep '^PYO3_PYTHON=' .env | cut -d '=' -f2-)
    export PYTHON_SYS_EXECUTABLE=$(grep '^PYTHON_SYS_EXECUTABLE=' .env | cut -d '=' -f2-)
fi

echo "Running all tests..."
cargo test --all

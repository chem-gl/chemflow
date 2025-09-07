#!/usr/bin/env bash
set -euo pipefail

# Cargar solo las variables necesarias del .env (si existen y no vacías)
if [ -f ".env" ]; then
    # LD_LIBRARY_PATH: anexar en vez de sobreescribir para evitar conflictos con libpq/libssl
    if grep -q '^LD_LIBRARY_PATH=' .env; then
        VAL=$(grep -m1 '^LD_LIBRARY_PATH=' .env | cut -d '=' -f2-)
        if [ -n "$VAL" ]; then
            export LD_LIBRARY_PATH="${LD_LIBRARY_PATH:-}:$VAL"
        fi
    fi
    if grep -q '^PYO3_PYTHON=' .env; then
        VAL=$(grep -m1 '^PYO3_PYTHON=' .env | cut -d '=' -f2-)
        if [ -n "$VAL" ]; then export PYO3_PYTHON="$VAL"; fi
    fi
    if grep -q '^PYTHON_SYS_EXECUTABLE=' .env; then
        VAL=$(grep -m1 '^PYTHON_SYS_EXECUTABLE=' .env | cut -d '=' -f2-)
        if [ -n "$VAL" ]; then export PYTHON_SYS_EXECUTABLE="$VAL"; fi
    fi
fi

cargo run "$@"

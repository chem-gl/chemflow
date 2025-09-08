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

# Mitigación opcional para libpq+GSS: si DATABASE_URL no especifica gssencmode y PGGSSENCMODE no está seteado, deshabilitar GSS
if [ -n "${DATABASE_URL:-}" ]; then
    if [ -z "${PGGSSENCMODE:-}" ] && ! echo "$DATABASE_URL" | grep -qi 'gssencmode='; then
        export PGGSSENCMODE=disable
        echo "[run.sh] PGGSSENCMODE=disable (auto)"
    fi
fi

cargo run "$@"

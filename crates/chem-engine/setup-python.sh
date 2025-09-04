#!/usr/bin/env bash
set -e

PY_VERSION=3.9.18
ENV_NAME=chem-env
ENV_FILE=".env"
REQUIREMENTS="requirements.txt"

if ! command -v pyenv &> /dev/null; then
  echo "pyenv no encontrado"
  exit 1
fi

# Instalar Python 3.9.18 con librería compartida
if ! pyenv versions --bare | grep -q "$PY_VERSION"; then
  PYTHON_CONFIGURE_OPTS="--enable-shared" pyenv install -s "$PY_VERSION"
fi

# Crear virtualenv si no existe
if ! pyenv virtualenvs --bare | grep -q "$ENV_NAME"; then
  pyenv virtualenv "$PY_VERSION" "$ENV_NAME"
fi

# Rutas del entorno
PYTHON_BIN="/home/cesar/.pyenv/versions/$PY_VERSION/bin/python3.9"
VENV_BIN="/home/cesar/.pyenv/versions/$ENV_NAME/bin"
PIP_BIN="$VENV_BIN/pip"
PY_LIB="/home/cesar/.pyenv/versions/$PY_VERSION/lib"

# Instalar pip y dependencias sin activar el entorno
"$PIP_BIN" install --upgrade pip
if [ -f "$REQUIREMENTS" ]; then
  "$PIP_BIN" install -r "$REQUIREMENTS"
fi

# Guardar rutas compatibles con PyO3 en .env
cat > "$ENV_FILE" <<EOF
LD_LIBRARY_PATH=$PY_LIB:\${LD_LIBRARY_PATH:-}
PYO3_PYTHON=$PYTHON_BIN
PYTHON_SYS_EXECUTABLE=$PYTHON_BIN
EOF

echo "Entorno listo. .env creado con variables para PyO3 y Rust:"
cat "$ENV_FILE"

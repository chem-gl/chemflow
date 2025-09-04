#!/usr/bin/env bash
set -e

# -----------------------------
# Configuración
# -----------------------------
PY_VERSION=3.9.18
ENV_NAME=chem-env
ENV_FILE=".env"
REQUIREMENTS="requirements.txt"

# -----------------------------
# Verificar pyenv
# -----------------------------
if ! command -v pyenv &> /dev/null; then
  echo "❌ No se encontró pyenv. Instálalo primero: https://github.com/pyenv/pyenv"
  exit 1
fi

# -----------------------------
# Instalar Python si no existe
# -----------------------------
if ! pyenv versions --bare | grep -q "$PY_VERSION"; then
  echo "➡️ Instalando Python $PY_VERSION..."
  pyenv install -s $PY_VERSION
fi

# -----------------------------
# Crear entorno virtual si no existe
# -----------------------------
if ! pyenv virtualenvs --bare | grep -q "$ENV_NAME"; then
  echo "➡️ Creando entorno virtual pyenv $ENV_NAME..."
  pyenv virtualenv $PY_VERSION $ENV_NAME
fi

# -----------------------------
# Activar entorno e instalar dependencias
# -----------------------------
echo "➡️ Activando entorno $ENV_NAME"
pyenv activate $ENV_NAME

# Verificar que exista requirements.txt
if [ ! -f "$REQUIREMENTS" ]; then
  echo "⚠️ No se encontró $REQUIREMENTS. Crealo con las librerías necesarias (ej. rdkit-pypi)."
else
  echo "➡️ Instalando dependencias desde $REQUIREMENTS..."
  pip install --upgrade pip
  pip install -r $REQUIREMENTS
fi

# -----------------------------
# Guardar ruta de Python en .env
# -----------------------------
PYTHON_PATH=$(pyenv which python)
echo "PYTHON_PATH=$PYTHON_PATH" > $ENV_FILE
echo "✅ Archivo $ENV_FILE creado con PYTHON_PATH=$PYTHON_PATH"

# -----------------------------
# Mensaje final
# -----------------------------
echo "✅ Entorno listo."
echo "Para desarrollo: pyenv activate $ENV_NAME"
echo "Para ejecutar tu binario Rust: asegúrate de leer PYTHON_PATH desde $ENV_FILE"

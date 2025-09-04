#!/usr/bin/env bash
set -euo pipefail

# Genera reporte de cobertura en formato Cobertura (XML)
echo "Running code coverage..."
cargo tarpaulin --out Xml

# Renombrar reporte generado a cobertura estandar
echo "Moving report to cobertura.xml..."
if [ -f tarpaulin-report.xml ]; then
  mv tarpaulin-report.xml cobertura.xml
  echo "Report available at cobertura.xml"
else
  echo "tarpaulin-report.xml not found" >&2
  exit 1
fi

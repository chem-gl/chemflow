#!/usr/bin/env bash
# Script para dividir diagramas-final.md en archivos por sección
# Genera archivos en documentation/secciones basados en los encabezados '## N. Título'

INPUT_FILE="$(dirname "$0")/../diagramas-final.md"
OUTPUT_DIR="$(dirname "$0")/secciones"

mkdir -p "$OUTPUT_DIR"
current_file=""

while IFS= read -r line; do
  if [[ "$line" =~ ^##[[:space:]]+([0-9]+)\.[[:space:]]+(.*) ]]; then
    num=${BASH_REMATCH[1]}
    title=${BASH_REMATCH[2]}
    # Formatear número con dos dígitos
    sec_num=$(printf "%02d" "$num")
    # Crear slug del título: minúsculas, espacios->-, eliminar caracteres inválidos
    slug=$(echo "$title" | tr '[:upper:]' '[:lower:]' | sed -E 's/[^a-z0-9]+/-/g' | sed -E 's/^-+|-+$//g')
    filename="$OUTPUT_DIR/${sec_num}-${slug}.md"
    current_file="$filename"
    # Escribir encabezado principal en el nuevo archivo
    echo "# Sección ${num} - ${title}" > "$current_file"
  elif [ -n "$current_file" ]; then
    echo "$line" >> "$current_file"
  fi
 done < "$INPUT_FILE"

 echo "División completada: archivos generados en $OUTPUT_DIR"

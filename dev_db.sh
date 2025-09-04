#!/usr/bin/env bash
set -euo pipefail

# Levanta la base de datos de desarrollo usando Docker Compose
echo "Starting development database..."
docker-compose -f postgress-docker/compose.yaml up -d --build

echo "Waiting for PostgreSQL to be ready..."
until docker exec postgres_db pg_isready -U admin > /dev/null 2>&1; do
  sleep 1
  echo -n "."
done

echo -e "\nDevelopment database is up and running!"

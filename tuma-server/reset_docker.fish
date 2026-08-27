# Nuke the dev database (container + volume) and start fresh.
cd (dirname (status -f))
docker compose down
docker container rm -f tuma-postgres-dev 2>/dev/null
docker volume rm tuma-server_pgdata 2>/dev/null
docker compose up -d

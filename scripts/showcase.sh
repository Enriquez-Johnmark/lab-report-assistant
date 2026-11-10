#!/usr/bin/env bash
# Makes the screenshots and the walkthrough in docs/showcase/ on a fresh
# database: one made-up clinic, its administrator and a physician, with the
# demo reports drafted from Claude's recorded answers (no key, no cost).
# Needs Postgres and Mailpit (docker compose up -d) and a built web app.
set -euo pipefail
cd "$(dirname "$0")/.."

db=lab_reports_showcase
export DATABASE_URL="postgres://lab_reports:lab_reports@localhost:5432/$db"
export MASTER_KEY="${MASTER_KEY:-0000000000000000000000000000000000000000000000000000000000000001}"
bin=target/debug/lab-report-assistant
cargo build --quiet
psql_admin() { docker compose exec -T postgres psql -q -U lab_reports -d postgres -c "$1"; }
psql_admin "drop database if exists $db"
psql_admin "create database $db"
$bin migrate >/dev/null
clinic=$($bin clinic add --name "Example Women's Health" --email admin@example.test)
$bin clinic add-physician --clinic "$clinic" --email dr.example@example.test --name "Dr. Example" >/dev/null
curl -s -X DELETE http://127.0.0.1:8025/api/v1/messages >/dev/null

BIND_ADDR=127.0.0.1:3002 WEB_DIR=web/dist $bin serve --recorded >target/showcase-server.log 2>&1 &
server=$!
trap 'kill $server' EXIT
for _ in $(seq 1 30); do curl -sf http://127.0.0.1:3002/health >/dev/null && break; sleep 1; done

(cd web && pnpm exec playwright test --config showcase/playwright.config.ts --trace off)

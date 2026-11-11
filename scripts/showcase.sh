#!/usr/bin/env bash
# Makes the screenshots and the walkthrough in docs/showcase/ on a fresh
# database: two made-up clinics, each with its administrator and a physician,
# the demo reports drafted from Claude's recorded answers (no key, no cost).
# Needs Postgres and Mailpit (docker compose up -d), a built web app, and
# pdftoppm (poppler-utils) for the PDF's picture; it starts HAPI FHIR itself.
set -euo pipefail
cd "$(dirname "$0")/.."

db=lab_reports_showcase
export DATABASE_URL="postgres://lab_reports:lab_reports@localhost:5432/$db"
export MASTER_KEY="${MASTER_KEY:-0000000000000000000000000000000000000000000000000000000000000001}"
bin=target/debug/lab-report-assistant
SQLX_OFFLINE=true cargo build --quiet
psql_admin() { docker compose exec -T postgres psql -q -U lab_reports -d postgres -c "$1"; }
psql_admin "drop database if exists $db"
psql_admin "create database $db"
$bin migrate >/dev/null
clinic=$($bin clinic add --name "Example Women's Health" --email admin@example.test)
$bin clinic add-physician --clinic "$clinic" --email dr.example@example.test --name "Dr. Example" >/dev/null
# A second made-up clinic, side by side (M9): its own staff, reports and FHIR server.
family=$($bin clinic add --name "Example Family Clinic" --email admin@family.example.test)
$bin clinic add-physician --clinic "$family" --email dr.family@family.example.test --name "Dr. Family" >/dev/null
# The first clinic's FHIR server (M8): HAPI FHIR on this computer, holding the
# 30 made-up reports. Loading them again changes nothing.
fhir=http://127.0.0.1:8090/fhir
curl -sf "$fhir/metadata" >/dev/null || docker compose --profile fhir up -d hapi
for _ in $(seq 1 90); do curl -sf "$fhir/metadata" >/dev/null && break; sleep 2; done
curl -sf "$fhir/metadata" >/dev/null || { echo "HAPI FHIR didn't start at $fhir" >&2; exit 1; }
$bin load-made-up --url "$fhir" >/dev/null
# A server already on that port is reused only if it holds just the 30
# made-up reports, so the walkthrough fetches exactly those.
reports=$(curl -sf "$fhir/DiagnosticReport?category=LAB&_summary=count" | grep -o '"total" *: *[0-9]*' | grep -o '[0-9]*$')
[ "$reports" = 30 ] || { echo "$fhir holds $reports lab reports, not the 30 made-up ones: stop it and run again" >&2; exit 1; }
$bin clinic set-fhir-server --clinic "$clinic" --url "$fhir" >/dev/null
curl -s -X DELETE http://127.0.0.1:8025/api/v1/messages >/dev/null

BIND_ADDR=127.0.0.1:3002 WEB_DIR=web/dist $bin serve --recorded >target/showcase-server.log 2>&1 &
server=$!
trap 'kill $server' EXIT
for _ in $(seq 1 30); do curl -sf http://127.0.0.1:3002/health >/dev/null && break; sleep 1; done

(cd web && pnpm exec playwright test --config showcase/playwright.config.ts --trace off)

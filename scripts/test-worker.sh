#!/usr/bin/env bash
# Exercise cf Build Output and cf dev without touching a developer's D1/secrets.
set -euo pipefail
cd "$(dirname "$0")/.."
root=$PWD
export CF_SEND_TELEMETRY=false
npx --no-install cf build
npx --no-install cf deploy --prebuilt --dry-run --no-provision
work=$(mktemp -d)
server=
cleanup() {
  if [[ -n "$server" ]]; then
    kill "$server" 2>/dev/null || true
    wait "$server" 2>/dev/null || true
  fi
  rm -rf "$work"
}
trap cleanup EXIT
cp cloudflare.config.ts wrangler.config.ts package.json "$work/"
ln -s "$root/node_modules" "$work/node_modules"
ln -s "$root/build" "$work/build"
cat > "$work/.dev.vars.test" <<'VARS'
GOOGLE_CLIENT_ID=ci-client.apps.example
GOOGLE_CLIENT_SECRET=ci-client-secret
GOOGLE_REDIRECT_URL=http://localhost:8791/auth/google/callback
VAPID_PUBLIC_KEY=ci-unused-public-key
VAPID_PRIVATE_KEY=ci-unused-private-key
VAPID_SUBJECT=mailto:ci@example.invalid
VARS
export PERSIST_DIR="$work/.wrangler/state"
npx --no-install cf d1 migrations apply 00000000-0000-4000-8000-000000000001 --local --persist-to "$PERSIST_DIR"
(cd "$work" && exec "$root/node_modules/.bin/cf" dev --mode test --port 8791) > "$work/dev.log" 2>&1 &
server=$!
ready=false
for _ in {1..60}; do
  if curl --silent --fail http://localhost:8791/manifest.webmanifest > /dev/null; then ready=true; break; fi
  if ! kill -0 "$server" 2>/dev/null; then break; fi
  sleep 1
done
if [[ "$ready" != true ]]; then cat "$work/dev.log"; exit 1; fi
if ! python3 scripts/test-worker.py; then cat "$work/dev.log"; exit 1; fi
# Local Explorer replaces --test-scheduled, which cf's Wrangler delegate rejects.
curl --fail --silent --show-error -X POST \
  'http://localhost:8791/cdn-cgi/local/explorer/api/local/scheduled?worker=app' \
  -H 'Content-Type: application/json' --data '{"cron":"* * * * *"}' > "$work/scheduled.json"
python3 - "$work/scheduled.json" <<'PY'
import json, sys
result = json.load(open(sys.argv[1]))
assert result.get("success") is True, result
print("Local scheduled invocation: OK")
PY

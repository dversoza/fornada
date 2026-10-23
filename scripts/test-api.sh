#!/usr/bin/env bash
# Builds, migrates a throwaway local D1 and runs tests/api.test.mjs against `wrangler dev`.
set -euo pipefail
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
cd "$(dirname "$0")/.."

persist="$(mktemp -d)"
log="$persist/dev.log"
dev_pid=""
cleanup() {
  [ -n "$dev_pid" ] && kill "$dev_pid" 2>/dev/null && wait "$dev_pid" 2>/dev/null || true
  pkill -P "$$" 2>/dev/null || true
  rm -rf "$persist"
}
trap cleanup EXIT

port="$(node -e 'const s=require("net").createServer().listen(0,()=>{console.log(s.address().port);s.close()})')"

npx wrangler d1 migrations apply DB --local --persist-to "$persist" >/dev/null
# wrangler dev runs the build command from wrangler.jsonc before serving.
npx wrangler dev --local --port "$port" --persist-to "$persist" >"$log" 2>&1 &
dev_pid=$!

for _ in $(seq 1 600); do
  if curl -fs -o /dev/null "http://127.0.0.1:$port/api/save"; then break; fi
  kill -0 "$dev_pid" 2>/dev/null || { cat "$log" >&2; echo "wrangler dev exited" >&2; exit 1; }
  sleep 0.5
done
curl -fs -o /dev/null "http://127.0.0.1:$port/api/save" || { cat "$log" >&2; echo "dev server not ready" >&2; exit 1; }

BASE_URL="http://127.0.0.1:$port" PERSIST_DIR="$persist" node --test tests/api.test.mjs || { tail -40 "$log" >&2; exit 1; }

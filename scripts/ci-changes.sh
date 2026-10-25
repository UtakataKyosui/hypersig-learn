#!/usr/bin/env bash
set -euo pipefail

# Unknown paths run everything. Embedded assets also require Rust validation.
rust=false
deploy=false
if ! git cat-file -e "${BASE_SHA}^{commit}" 2>/dev/null; then
  # Initial push or unavailable history: never silently skip validation.
  rust=true
  deploy=true
else
  base=$BASE_SHA
  if [[ $EVENT_NAME == pull_request ]]; then
    base=$(git merge-base "$BASE_SHA" "$HEAD_SHA")
  fi
  changed=$(mktemp)
  trap 'rm -f "$changed"' EXIT
  # Disable rename detection so moving code into docs still checks the deletion.
  git diff --no-renames --name-only -z "$base" "$HEAD_SHA" > "$changed"
  while IFS= read -r -d '' path; do
    case "$path" in
      docs/*|README.md|AGENTS.md|LICENSE|LICENSE.*|.github/dependabot.yml) ;;
      wrangler.toml) deploy=true ;;
      *) rust=true; deploy=true ;;
    esac
  done < "$changed"
fi
printf 'rust=%s\ndeploy=%s\n' "$rust" "$deploy" >> "$GITHUB_OUTPUT"

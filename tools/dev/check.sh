#!/usr/bin/env bash
# Reproducible checks; no deployment, root policy edits or credential defaults.
set -euo pipefail
tealdrive_tool_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
tealdrive_repo_dir=$(cd -- "$tealdrive_tool_dir/../.." && pwd)
cd -- "$tealdrive_repo_dir"
case "${1:-}" in
  unit)
    cargo fmt --all -- --check
    cargo clippy --locked --all-targets -- -D warnings
    cargo test --locked
    ;;
  proxy-admin)
    exec python3 tools/dev/proxy-admin-check.py
    ;;
  wizard)
    exec python3 tools/dev/wizard-acceptance.py
    ;;
  admin)
    exec python3 tools/dev/admin-acceptance.py
    ;;
  live)
    case "${2:-}" in
      ui-auth|ui-credentials|sdk-access|sdk-enrollment|sdk-suspend) tealdrive_script=$2 ;;
      *) echo 'Choose an explicit live test: ui-auth, ui-credentials, sdk-access, sdk-enrollment, sdk-suspend.' >&2; exit 2 ;;
    esac
    : "${TEALDRIVE_TEST_URL:?Explicit dedicated test URL required}"
    : "${TEALDRIVE_TEST_USERNAME:?Dedicated test account required}"
    : "${TEALDRIVE_TEST_WEB_PASSWORD_FILE:?Private web password file required}"
    export NODE_PATH="$tealdrive_tool_dir/node_modules${NODE_PATH:+:$NODE_PATH}"
    exec node "tests/live/$tealdrive_script.cjs"
    ;;
  *) echo 'Usage: tools/dev/check.sh unit | wizard | admin | proxy-admin | live <test-name>' >&2; exit 2 ;;
esac

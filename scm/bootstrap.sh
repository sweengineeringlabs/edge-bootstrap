#!/usr/bin/env bash
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
echo "==> Installing git hooks"
git -C "$REPO_ROOT" config core.hooksPath scm/scripts/hooks
echo "==> Fetching dependencies"
(cd "$REPO_ROOT/scm" && cargo fetch --locked)
echo "Bootstrap complete."

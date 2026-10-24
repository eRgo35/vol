#!/usr/bin/env bash
# Points git at the versioned hooks in scripts/hooks.
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

chmod +x scripts/hooks/*
git config core.hooksPath scripts/hooks
echo "git hooks installed: core.hooksPath=scripts/hooks"

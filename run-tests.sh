#!/usr/bin/env bash
set -euo pipefail

case "${1:-all}" in
    unit | all)
        cargo test "$@"
        ;;
    *)
        cargo test "$@"
        ;;
esac

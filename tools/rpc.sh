#!/usr/bin/env bash
# Override the firmware's default Thumb target for the native host client.
set -euo pipefail
repo=$(cd -- "$(dirname -- "$0")/.." && pwd)
host_target=$(rustc -vV | sed -n 's/^host: //p')
exec cargo run --quiet --manifest-path "$repo/tools/km003c-rpc/Cargo.toml" --target "$host_target" -- "$@"

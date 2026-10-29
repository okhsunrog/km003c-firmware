set positional-arguments

# Host tools share a workspace; the device has its own target/config/lockfile.
default:
    @just --list

check: check-host check-firmware

check-host:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo test --workspace --locked

check-firmware:
    cd firmware && cargo fmt -- --check
    cd firmware && cargo clippy --release --locked -- -D warnings
    cd firmware && cargo build --release --locked

fmt:
    cargo fmt --all
    cd firmware && cargo fmt

build: build-firmware

build-firmware:
    cd firmware && cargo build --release --locked

cli *args:
    cargo run --quiet --locked -p km003c-host-cli -- "$@"

# Package a release image offline. Output files are never overwritten.
package output="artifacts/km003c.mencrypt":
    mkdir -p artifacts
    cd firmware && cargo objcopy --release --locked --bin km003c-firmware -- -O binary ../artifacts/km003c.bin
    just cli package artifacts/km003c.bin "$1"

# Flash requires a saved matching backup and a NEW evidence directory.
flash image backup evidence:
    just cli flash "$1" --backup "$2" --evidence "$3"

backup output:
    just cli backup "$1"

verify image:
    just cli verify "$1"

start:
    just cli start

loader:
    just cli loader

diagnostics output:
    just cli diagnostics "$1"

maneuver plan output:
    just cli maneuver "$1" --output "$2"

#!/usr/bin/env bash

set -eux

export RUSTFLAGS="--deny warnings"

cargo build --all
cargo test --all
cargo clippy --all
cargo fmt --all -- --check

#!/usr/bin/env bash

set -eux

cargo build --all
cargo test --all
cargo clippy --all
cargo fmt --all -- --check

all: build test clippy fmt-check

build:
  cargo build --all

test:
  cargo test --all

clippy:
  cargo clippy --all

fmt-check:
  cargo fmt --all -- --check
  @echo formatting check done

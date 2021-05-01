all: build test smoke clippy fmt-check forbid

build:
  cargo build --all

test:
  cargo test --all

smoke:
  cargo test --test smoke

clippy:
  cargo clippy --all

fmt-check:
  cargo fmt --all -- --check
  @echo formatting check done

forbid:
  ./bin/forbid

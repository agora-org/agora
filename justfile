all: build test smoke clippy fmt-check forbid

build:
  cargo build --all

test pattern='':
  cargo test --all {{pattern}}

smoke:
  cargo test --test smoke

clippy:
  cargo clippy --all-targets --all-features

fmt-check:
  cargo fmt --all -- --check
  @echo formatting check done

forbid:
  ./bin/forbid

check-install:
  #!/usr/bin/env bash
  tmp=`mktemp -d`
  cargo install --path . --root $tmp
  $tmp/bin/foo --version

watch +command='test':
	cargo watch --exec '{{command}}'

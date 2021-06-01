all: build test smoke clippy fmt-check forbid check-install

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
  $tmp/bin/agora --version

watch +command='test':
	cargo watch --exec '{{command}}'

version := `sed -En 's/version[[:space:]]*=[[:space:]]*"([^"]+)"/\1/p' Cargo.toml | head -1`

publish remote: all
	git diff --no-ext-diff --quiet --exit-code
	git branch | grep '* master'
	cargo publish --dry-run
	git tag -a {{version}} -m 'Release version {{version}}'
	git push {{remote}} {{version}}
	cargo publish

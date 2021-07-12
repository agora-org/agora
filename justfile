all: build test smoke clippy fmt-check forbid check-install check-lockfile

build:
  cargo check --all
  cargo check --tests
  cargo check --tests --all-features

test pattern='':
  cargo test --all --all-features {{pattern}}

smoke +args="":
  cargo test --test smoke {{args}}

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

check-lockfile:
  cargo update --locked --package agora

watch +command='test':
  cargo watch --exec '{{command}}'

publish remote: all
  #!/usr/bin/env bash
  set -euxo pipefail
  VERSION=`cargo run -- --version | cut -d' ' -f2`
  git diff --no-ext-diff --quiet --exit-code
  git branch | grep '* master'
  cargo publish --dry-run
  git tag -a $VERSION -m "Release version $VERSION"
  git push {{remote}} $VERSION
  cargo publish

clean-binaries:
  rm -rf target/bitcoin* target/ln*

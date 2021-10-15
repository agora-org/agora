set positional-arguments

all: build audit test integration clippy fmt-check forbid check-lockfile

audit:
  cargo audit

build:
  cargo lcheck --all
  cargo lcheck --tests
  cargo lcheck --tests --all-features

test *args="--all":
  cargo ltest --all-features "$@"

fast-tests *args="":
  cargo ltest "$@"

slow-tests *args="slow_tests":
  cargo ltest --features slow-tests "$@"

integration *args:
  cargo ltest --test integration "$@"

clippy:
  cargo lclippy --all-targets --all-features
  cargo lclippy --all-targets --all-features --tests

fmt-check:
  cargo fmt --all -- --check
  @echo formatting check done

forbid:
  ./bin/forbid

check-lockfile:
  cargo update --locked --package agora

watch +command='ltest':
  cargo watch --clear --exec '{{command}}'

push: all
  git push

publish revision:
  cargo run -p publish -- {{revision}}

clean-binaries:
  rm -rf target/bitcoin* target/ln*

run example-files='example-files' domain='test.agora.download' network='testnet':
  cargo lcheck
  scp root@{{domain}}:/var/lib/lnd/tls.cert target/tls.cert
  scp root@{{domain}}:/var/lib/lnd/data/chain/bitcoin/{{network}}/invoice.macaroon target/invoice.macaroon
  cargo lrun -- \
    --address localhost \
    --http-port 8080 \
    --directory {{example-files}} \
    --lnd-rpc-authority {{domain}}:10009 \
    --lnd-rpc-cert-path target/tls.cert \
    --lnd-rpc-macaroon-path target/invoice.macaroon

open:
  #!/usr/bin/env bash
  set -euo pipefail
  if command -v xdg-open &> /dev/null; then
    xdg-open http://localhost:8080
  else
    open http://localhost:8080
  fi

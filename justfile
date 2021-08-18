set positional-arguments

all: build test smoke clippy fmt-check forbid check-install check-lockfile

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

smoke *args:
  cargo ltest --test smoke "$@"

clippy:
  cargo lclippy --all-targets --all-features

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

watch +command='ltest':
  cargo watch --exec '{{command}}'

push: all
  git push

publish revision:
  cargo run -p publish -- {{revision}}

clean-binaries:
  rm -rf target/bitcoin* target/ln*

run domain='test.agora.download' network='testnet':
  scp root@{{domain}}:/var/lib/lnd/tls.cert target/tls.cert
  scp root@{{domain}}:/var/lib/lnd/data/chain/bitcoin/{{network}}/invoice.macaroon target/invoice.macaroon
  cargo run -- \
    --address localhost \
    --port 8080 \
    --directory example-files \
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

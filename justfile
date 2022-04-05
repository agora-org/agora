set positional-arguments

all: build test clippy fmt-check forbid check-lockfile

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

render-icons:
  mkdir -p tmp
  rsvg-convert \
    --output tmp/favicon.png \
    --zoom 32 \
    logo.svg
  convert tmp/favicon.png static/favicon.ico
  rsvg-convert \
    --background-color '#3457d5' \
    --output static/apple-touch-icon.png \
    --zoom 180 \
    logo.svg

open:
  #!/usr/bin/env bash
  set -euo pipefail
  if command -v xdg-open &> /dev/null; then
    xdg-open http://localhost:8080
  else
    open http://localhost:8080
  fi

scrape-website:
  wget \
    --adjust-extension \
    --convert-links \
    --directory-prefix docs \
    --mirror \
    --no-host-directories \
    --page-requisites \
    https://agora.download/

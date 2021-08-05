set positional-arguments

all: build test smoke clippy fmt-check forbid check-install check-lockfile

build:
  cargo lcheck --all
  cargo lcheck --tests
  cargo lcheck --tests --all-features

test *args="--all":
  cargo ltest "$@"
  cargo ltest --all-features "$@"

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

publish remote: all
  #!/usr/bin/env bash
  set -euxo pipefail
  VERSION=`cargo run -- --version | cut -d' ' -f2`
  git diff --no-ext-diff --quiet --exit-code
  git branch | grep '* master'
  (cd agora-lnd-client && cargo publish --dry-run)
  cargo publish --dry-run
  git tag -a $VERSION -m "Release version $VERSION"
  git push {{remote}} $VERSION
  (cd agora-lnd-client && cargo publish)
  cargo publish

clean-binaries:
  rm -rf target/bitcoin* target/ln*

run domain:
  scp root@{{domain}}:/var/lib/lnd/tls.cert target/tls.cert
  scp root@{{domain}}:/var/lib/lnd/data/chain/bitcoin/testnet/invoice.macaroon target/invoice.macaroon
  cargo run -- \
    --lnd-rpc-authority {{domain}}:10009 \
    --lnd-rpc-cert-path target/tls.cert \
    --lnd-rpc-macaroon-path target/invoice.macaroon \
    --directory .

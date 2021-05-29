# `agora`

`agora` serves the contents of a local directory, providing file listings and downloads over HTTP.
For example, you can point it at a directory full of PDFs, allowing users to browse and view the PDFs in their web browser.

## Running

```bash
$ mkdir files
$ echo 'amazing content' > files/file.txt
$ agora --direcory files --port 1234
$ curl http://localhost:1234/files/file.txt
```

See `agora --help` for more configuration options.

## Building

`agora` is written in [Rust](https://www.rust-lang.org/) and built with `cargo`.
You can install Rust with [rustup](https://rustup.rs/).

Inside the checked out repository, running `cargo build --release` will build `agora` and copy the binary to `./target/release/agora`.

From within the repository, you can also run, e.g., `cargo install --path . --root /usr/local`, which will copy `agora` to `/usr/local/bin/agora`.

## Deployment

The `agora` binary contains its static assets, so it can be copied and run from anywhere on the filesystem.
By default `cargo` links to system libraries dynamically.
You can avoid this by using the `x86_64-unknown-linux-musl` target: `cargo build --target=x86_64-unknown-linux-musl --release`.
This produces a statically linked binary that runs on, e.g., Alpine and CentOS Linux.

## Development

You can run the tests locally with `cargo test`.
Pull requests are tested on github actions, with the workflow defined in `.github/workflows/build.yaml`.
You can run approximately the same tests locally with `just all`.
(See [just](https://github.com/casey/just).)

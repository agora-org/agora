# `foo`

`foo` serves the contents of a local directory, serving file listings and downloads over HTTP. For example, you can point it at a directory full of PDFs, allowing users to browse and view PDFs in their web browser.

## Running

```
$ mkdir files
$ echo 'amazing content' > files/file.txt
$ foo --direcory files --port 1234
$ curl http://localhost:1234/files/file.txt
```

See `foo --help` for more configuration options.

## Building

`foo` is written in Rust and is built with Cargo. Inside the checked out repository, running `cargo build --release` will build `foo` and copy the binary to `target/release/foo`.

From within the repository, you can also run e.g. `cargo install --path . --root /usr/local`, which will copy `foo` to `/usr/local/bin/foo`.

## Deployment

The `foo` binary contains its static assets. So it can be copied and run from anywhere on the filesystem.
By default `cargo` links to some system libraries dynamically.
You can avoid this by using the `x86_64-unknown-linux-musl` target: `cargo build --target=x86_64-unknown-linux-musl --release`.
This produces a statically linked binary that runs on e.g. Alpine and CentOS Linux.

## Development

You can run the tests locally with `cargo test`.
Pull requests are tested on github actions, with the workflow in `.github/workflows/build.yaml`.
You can approximately the same tests that run on CI locally with `just all`.
(See [just](https://github.com/casey/just).)

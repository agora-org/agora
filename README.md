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

`foo` is written in Rust and is built with Cargo. Running `cargo build --release` will compile the binary in `target/release/foo`.

You can also run `cargo install

## Deployment

- self contained (no runtime files, but needs libs)
- copy and run

## development

- cargo test
- something about running the CI tests

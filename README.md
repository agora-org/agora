# `agora`

`agora` serves the contents of a local directory, providing file listings and downloads over HTTP.
For example, you can point it at a directory full of PDFs, allowing users to browse and view the PDFs in their web browser.

## Running

```bash
$ mkdir files
$ echo 'amazing content' > files/file.txt
$ agora --directory files --port 1234
$ curl http://localhost:1234/files/file.txt
```

See `agora --help` for more configuration options.

## Installation

Pre-built binaries for Linux, MacOS, and Windows can be found on [the releases page](https://github.com/soenkehahn/agora/releases).

## Building from Source

`agora` is written in [Rust](https://www.rust-lang.org/) and built with `cargo`.
You can install Rust with [rustup](https://rustup.rs/).

Inside the checked out repository, running `cargo build --release` will build `agora` and copy the binary to `./target/release/agora`.

From within the repository, you can also run, e.g., `cargo install --path . --root /usr/local`, which will copy `agora` to `/usr/local/bin/agora`.

## Deployment

The `agora` binary contains its static assets, so it can be copied and run from anywhere on the filesystem.
By default `cargo` links to system libraries dynamically.
You can avoid this by using the `x86_64-unknown-linux-musl` target: `cargo build --target=x86_64-unknown-linux-musl --release`.
This produces a statically linked binary that runs on, e.g., Alpine and CentOS Linux.

### Configuration

You can configure what network port and addresses `agora` will be listening on, and what directory it will serve.
See `agora --help` for details.

### LND Configuration

By default `agora` will serve files for free.
In order to be able to charge for downloads, `agora` must be connected to an [LND](https://github.com/lightningnetwork/lnd) instance.
There's multiple command line flags to configure this connection, see `agora --help` for details.

To configure which files are free and which are paid, see [Access Configuration](#access-configuration) below.

### Access Configuration

You can put an `.agora.yaml` configuration file into directories served by `agora` to configure access to the files in that directory.
Currently, access configuration does not apply recursively to files in subdirectories,
so you'll need a `.agora.yaml` file in every directory you want to configure.

The default configuration is:

```yaml
# whether or not to charge for files
paid: false
```

Currently, `agora` charges the low low price of 1000 satoshis for all paid files.

## Development

You can run the tests locally with `cargo test`.
Pull requests are tested on github actions, with the workflow defined in `.github/workflows/build.yaml`.
You can run approximately the same tests locally with `just all`.
(See [just](https://github.com/casey/just).)

## License

Agora is licensed under [the CC0](https://choosealicense.com/licenses/cc0-1.0) with the exception of third-party components listed in [`ATTRIBUTION.md`](ATTRIBUTION.md).

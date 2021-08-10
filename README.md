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

You can configure the network port and address `agora` listens on, and the directory it serves.
See `agora --help` for details.

### LND Configuration

By default `agora` serves files for free.
To charge for downloads, `agora` must be connected to an [LND](https://github.com/lightningnetwork/lnd) instance.
There are multiple command line flags to configure this connection, see `agora --help` for details.

To configure which files are free and which are paid, see [Access Configuration](#access-configuration) below.

### Access Configuration

You can put a `.agora.yaml` configuration file into directories served by `agora` to configure access to files in that directory.
Currently, access configuration does not apply recursively to files in subdirectories,
so you'll need a `.agora.yaml` file in every directory you want to configure.

The default configuration is:

```yaml
# whether or not to charge for files
paid: false
```

Currently, `agora` charges the low low price of 1,000 satoshis for all paid files.

## Development Agora Instances

There are Agora instances accessible at [agora.download](http://agora.download) and [test.agora.download](http://test.agora.download). [agora.download](http://agora.download) operates on the Bitcoin mainnet, and the invoices it generates can be paid with any mainnet Lightning Network wallet. [http://test.agora.download](http://test.agora.download) operates on the Bitcoin testnet, and the invoices it generates can only be paid with a testnet Lightning Network wallet, for example, [htlc.me](https://htlc.me).

## Development

You can run the tests locally with `cargo test`.
Pull requests are tested on github actions, with the workflow defined in `.github/workflows/build.yaml`.
You can run approximately the same tests locally with `just all`.
(See [just](https://github.com/casey/just).)

## License

Agora is licensed under [the CC0](https://choosealicense.com/licenses/cc0-1.0) with the exception of third-party components listed in [`ATTRIBUTION.md`](ATTRIBUTION.md).

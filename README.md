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

### Custom Index Pages

`agora` serves directory file listings.
If a `.index.md` file is present in a directory, `agora` will render the contained Markdown as HTML and include it with the file listing. `agora` expects Commonmark Markdown, extended with footnotes, [strikethrough](https://github.github.com/gfm/#strikethrough-extension-), [tables](https://github.github.com/gfm/#tables-extension-), and [task lists](https://github.github.com/gfm/#task-list-items-extension-).

## Development Agora Instances

There are Agora instances accessible at [agora.download](http://agora.download) and [test.agora.download](http://test.agora.download).
[agora.download](http://agora.download) operates on the Bitcoin mainnet, and the invoices it generates can be paid with any **mainnet** Lightning Network wallet.
[http://test.agora.download](http://test.agora.download) operates on the Bitcoin testnet, and the invoices it generates can only be paid with a **testnet** Lightning Network wallet, for example, [htlc.me](https://htlc.me/).

## Buying Files from an Agora Instance

You can navigate to any Agora instance and browse the hosted files.
Agora instances can host a mix of free and paid files.
For paid files, Agora will present you a Lightning Network invoice
that you must pay before downloading the file.
These invoices can be paid with a Lightning Network wallet.
Popular wallets include:

- [Wallet of Satoshi](https://www.walletofsatoshi.com/), a hosted wallet for Android and iOS.
- [Strike](https://strike.me/), a hosted wallet for Android, iOS, and Google Chrome.
- [Muun](https://muun.com/), a self-custodial wallet for iOS and Android.
- [Breez](https://breez.technology/), a self-custodial wallet for iOS and Android.
- [River Financial](https://river.com/), a Bitcoin financial services platform with the ability to buy and sell bitcoin for USD, and make and receive Lightning Payments, for the web, iOS, and Android.

## Selling Files with Agora

Agora is not a hosted platform.
If you want to sell files through it, you'll have to host your own Agora instance.
Agora instances require access to an [LND](https://github.com/lightningnetwork/lnd) instance
to create invoices and query their payment status.
LND in turn needs access to a bitcoin node
-- e.g. [`bitcoind`](https://github.com/bitcoin/bitcoin/) --
to query the state of the bitcoin blockchain.

### Setting up `bitcoind` and LND

Setting up `bitcoind` and LND is a complex topic, and many different approaches are possible.
An excellent guide to setting up LND on Linux is available [here](https://stopanddecrypt.medium.com/a-complete-beginners-guide-to-installing-a-lightning-node-on-linux-2021-edition-ece227cfc35d),
and a companion guide to setting up `bitcoind`,
to supply the Lightning Network node with information about the blockchain,
is [here](https://stopanddecrypt.medium.com/a-complete-beginners-guide-to-installing-a-bitcoin-full-node-on-linux-2021-edition-46bf20fbe8ff).

### Processing Payments with Agora

In order to process payments, Agora needs to be connected to an LND instance.
See the `--lnd-*` flags in `agora --help`.

Additionally, LND nodes can only receive payments if they have sufficient _inbound liquidity_.

#### Inbound Liquidity

Liquidity management is one of the most complicated aspects of the Lightning Network, and certainly one of the most counter-intuitive.

The basic primitive that makes up the Lightning Network is the "payment channel", commonly referred to as just a "channel".
A channel is between two Lightning Network nodes, has a fixed capacity, is opened by making a on-chain Bitcoin transaction, and is closed by making an on-chain Bitcoin transaction.

While the channel is open, the two parties to the channel can make payments between themselves, but do not have to publish a Bitcoin transaction for each one of these payments.
They only have to publish a Bitcoin transaction when they want to close the channel, which nets-out the intermediate transactions made since it was opened.

As a concrete example, let's say Alice and Bob open a channel, with Alice contributing 1 BTC when the channel is opened, and Bob contributing 0 BTC. Initially, their balances on the channel are:

    Alice: 1 BTC
    Bob:   0 BTC

In this state, Alice can send a 0.1 BTC payment to Bob, and the channel balances will be:

    Alice: 0.9 BTC
    Bob:   0.1 BTC

Alice and Bob can send each other money, but only up to the amount that they have on their side of the channel.
At this point, Alice can send Bob up to 0.9 BTC, and Bob can send Alice up to 0.1 BTC.

However, at the very beginning, when Bob's balance was 0 BTC, Alice could not have received any money from Bob.
Due to a lack of inbound liquidity, which is, quite simply, money on the other side of a channel, which can be sent to you.

This is an aspect of the Lightning Network that is very different from other payment systems, and from on-chain Bitcoin payments.
You must arrange to have sufficient inbound liquidity to receive payments.

A question you might ask is, _What if I just want to assume that the customer is good for the money and queue it up myself for settlement once I have the liquidity to spare?_

Let's imagine that we were in the initial state, Alice had 1 BTC in the channel, Bob 0 BTC, and Alice let Bob make a payment to her of 1 BTC. The new balance would be:

    Alice:  2 BTC
    Bob:   -1 BTC

However! When a Lightning Network channel is closed, you divide up the funds from the funding transaction between the parties to the channel.
The channel was funded with an on-chain Bitcoin transaction of 1 BTC, so there is no way to pay out 2 BTC to Alice from the initial funding transaction.
Since both parties to a payment channel can close the channel at any time, Alice would be trusting Bob to keep the channel open until he no longer had a negative balance.
This is not scalable or secure, and avoiding the need for trust is the whole purpose of the Lightning Network in the first place, otherwise we could all just trade unenforceable IOUs back and forth.

Inbound liquidity can be sourced in a number of ways, and [in-development proposals](https://github.com/lightningnetwork/lightning-rfc/pull/878) should make it even easier in the future.
For now, we recommend purchasing inbound liquidity from [Bitrefill](https://www.bitrefill.com/buy/lightning-channel/).
Bitrefill offers a service where a Lightning Node operator can pay Bitrefill to open a channel with that operator's Lightning Node.
The operator pays Bitrefill a small amount of bitcoin and receives a channel with a much greater amount of inbound liquidity in return.

## Development

You can run the tests locally with `cargo test`.
Pull requests are tested on github actions, with the workflow defined in `.github/workflows/build.yaml`.
You can run approximately the same tests locally with `just all`.
(See [just](https://github.com/casey/just).)

## License

Agora is licensed under [the CC0](https://choosealicense.com/licenses/cc0-1.0) with the exception of third-party components listed in [`ATTRIBUTION.md`](ATTRIBUTION.md).

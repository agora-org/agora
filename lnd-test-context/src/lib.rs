use crate::owned_child::{CommandExt, OwnedChild};
use cradle::*;
use lazy_static::lazy_static;
use std::{
  collections::BTreeSet,
  fs,
  net::TcpListener,
  path::{Path, PathBuf},
  process::Command,
  sync::Arc,
  time::Duration,
};
use tempfile::TempDir;
use tokio::sync::Mutex;

mod executables;
mod owned_child;

#[derive(Debug, Clone)]
pub struct LndTestContext {
  #[allow(unused)]
  bitcoind: OwnedChild,
  bitcoind_peer_port: u16,
  bitcoind_rpc_port: u16,
  #[allow(unused)]
  lnd: OwnedChild,
  lnd_peer_port: u16,
  pub lnd_rpc_port: u16,
  tmpdir: Arc<TempDir>,
}

impl LndTestContext {
  async fn guess_free_port() -> u16 {
    lazy_static! {
      static ref SET: Mutex<BTreeSet<u16>> = Mutex::const_new(BTreeSet::new());
    }
    let mut set = SET.lock().await;
    loop {
      let port = TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
      if !set.contains(&port) {
        set.insert(port);
        return port;
      }
    }
  }

  pub async fn new() -> Self {
    let tmpdir = tempfile::tempdir().unwrap();

    let bitcoinddir = tmpdir.path().join("bitcoind");

    fs::create_dir(&bitcoinddir).unwrap();
    fs::write(bitcoinddir.join("bitcoin.conf"), "\n").unwrap();

    let bitcoind_peer_port = Self::guess_free_port().await;
    let bitcoind_rpc_port = Self::guess_free_port().await;
    let zmqpubrawblock = Self::guess_free_port().await;
    let zmqpubrawtx = Self::guess_free_port().await;
    let bitcoind = Command::new(executables::bitcoind().await)
      .arg("-chain=regtest")
      .arg(format!("-datadir={}", bitcoinddir.to_str().unwrap()))
      .arg(format!("-rpcport={}", bitcoind_rpc_port))
      .arg("-rpcuser=user")
      .arg("-rpcpassword=password")
      .arg(format!("-port={}", bitcoind_peer_port))
      .arg(format!(
        "-bind=127.0.0.1:{}=onion",
        Self::guess_free_port().await
      ))
      .arg(format!(
        "-zmqpubrawblock=tcp://127.0.0.1:{}",
        zmqpubrawblock
      ))
      .arg(format!("-zmqpubrawtx=tcp://127.0.0.1:{}", zmqpubrawtx))
      .stdout(std::process::Stdio::null())
      .spawn_owned()
      .unwrap();

    let lnddir = tmpdir.path().join("lnd");

    let lnd_peer_port = Self::guess_free_port().await;
    let lnd_rpc_port = Self::guess_free_port().await;

    let lnd = 'outer: loop {
      let lnd = Command::new(executables::lnd().await)
        .args(&[
          "--bitcoin.regtest",
          "--bitcoin.active",
          "--bitcoin.node=bitcoind",
        ])
        .arg("--lnddir")
        .arg(&lnddir)
        .arg("--bitcoind.dir")
        .arg(&bitcoinddir)
        .arg(format!(
          "--bitcoind.rpchost=127.0.0.1:{}",
          bitcoind_rpc_port
        ))
        .arg("--bitcoind.rpcuser=user")
        .arg("--bitcoind.rpcpass=password")
        .arg(format!(
          "--bitcoind.zmqpubrawblock=127.0.0.1:{}",
          zmqpubrawblock
        ))
        .arg(format!("--bitcoind.zmqpubrawtx=127.0.0.1:{}", zmqpubrawtx))
        .arg("--debuglevel=trace")
        .arg("--noseedbackup")
        .arg("--norest")
        .arg(format!("--rpclisten=127.0.0.1:{}", lnd_rpc_port))
        .arg(format!("--listen=127.0.0.1:{}", lnd_peer_port))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn_owned()
        .unwrap();
      loop {
        let (Exit(status), Stderr(_), StdoutTrimmed(_)) = cmd!(
          Self::lncli_command_static(&lnddir, lnd_rpc_port).await,
          "getinfo"
        );
        if status.success() {
          break 'outer lnd;
        } else if lnd.inner.lock().unwrap().try_wait().unwrap().is_some() {
          break;
        } else {
          tokio::time::sleep(Duration::from_millis(50)).await;
        }
      }
    };

    let StdoutUntrimmed(_) = cmd!(
      Self::bitcoin_cli_command_static(&bitcoinddir, bitcoind_rpc_port).await,
      %"createwallet wallet_name=bitcoin-core-test-wallet"
    );

    Self {
      bitcoind,
      bitcoind_peer_port,
      bitcoind_rpc_port,
      lnd,
      lnd_peer_port,
      lnd_rpc_port,
      tmpdir: Arc::new(tmpdir),
    }
  }

  pub fn new_blocking() -> Self {
    tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .unwrap()
      .block_on(LndTestContext::new())
  }

  fn bitcoind_dir(&self) -> String {
    self
      .tmpdir
      .path()
      .join("bitcoind")
      .to_str()
      .unwrap()
      .to_string()
  }

  pub fn lnd_dir(&self) -> PathBuf {
    self.tmpdir.path().join("lnd")
  }

  pub fn lnd_rpc_authority(&self) -> String {
    format!("localhost:{}", self.lnd_rpc_port)
  }

  pub fn cert_path(&self) -> PathBuf {
    self.lnd_dir().join("tls.cert")
  }

  pub fn invoice_macaroon_path(&self) -> PathBuf {
    self
      .lnd_dir()
      .join("data/chain/bitcoin/regtest/invoice.macaroon")
  }

  async fn bitcoin_cli_command_static(bitcoind_dir: &Path, bitcoind_rpc_port: u16) -> Vec<String> {
    vec![
      executables::bitcoin_cli()
        .await
        .to_str()
        .unwrap()
        .to_string(),
      "-chain=regtest".to_string(),
      format!("-datadir={}", bitcoind_dir.to_str().unwrap()),
      format!("-rpcport={}", bitcoind_rpc_port),
      "-rpcuser=user".to_string(),
      "-rpcpassword=password".to_string(),
      "-named".to_string(),
    ]
  }

  async fn bitcoin_cli_command(&self) -> Vec<String> {
    Self::bitcoin_cli_command_static(Path::new(&self.bitcoind_dir()), self.bitcoind_rpc_port).await
  }

  pub async fn lncli_command_static(lnd_dir: &Path, lnd_rpc_port: u16) -> Vec<String> {
    vec![
      executables::lncli().await.to_str().unwrap().to_string(),
      "--network".to_string(),
      "regtest".to_string(),
      "--lnddir".to_string(),
      lnd_dir.to_str().unwrap().to_string(),
      "--rpcserver".to_string(),
      format!("localhost:{}", lnd_rpc_port),
    ]
  }

  pub async fn lncli_command(&self) -> Vec<String> {
    Self::lncli_command_static(&self.lnd_dir(), self.lnd_rpc_port).await
  }

  pub async fn run_lncli_command<I: cradle::Input>(&self, input: I) -> serde_json::Value {
    let (Exit(status), StdoutUntrimmed(output)) = cmd!(self.lncli_command().await, input);
    if !status.success() {
      eprintln!("{}", output);
      panic!("LndTestContext::run_lncli_command failed");
    }
    match serde_json::from_str(&output) {
      Ok(value) => value,
      Err(_) => {
        eprintln!("{}", output);
        panic!("LndTestContext::run_lncli_command: failed to parse json");
      }
    }
  }

  async fn mine_blocks(&self, n: i32) {
    let StdoutTrimmed(address) = cmd!(self.bitcoin_cli_command().await, "getnewaddress");

    let StdoutUntrimmed(_) = cmd!(
      self.bitcoin_cli_command().await,
      "generatetoaddress",
      format!("nblocks={}", n),
      format!("address={}", address),
    );
  }

  async fn generate_bitcoind_wallet_with_money(&self) {
    self.mine_blocks(101).await;
  }

  async fn wait_to_sync(&self) {
    while !self.run_lncli_command("getinfo").await["synced_to_chain"]
      .as_bool()
      .unwrap()
    {
      tokio::time::sleep(Duration::from_millis(50)).await;
    }
  }

  pub async fn generate_money_into_lnd(&self) {
    self.generate_bitcoind_wallet_with_money().await;
    let lnd_new_address = self.run_lncli_command(("newaddress", "p2wkh")).await["address"]
      .as_str()
      .unwrap()
      .to_string();
    let StdoutUntrimmed(_) = cmd!(
      self.bitcoin_cli_command().await,
      %"sendtoaddress amount=2 fee_rate=100",
      format!("address={}", &lnd_new_address),
    );
    self.mine_blocks(1).await;
    self.wait_to_sync().await;
  }

  async fn connect_bitcoinds(&self, other: &LndTestContext) {
    cmd_unit!(
      self.bitcoin_cli_command().await,
      "addnode",
      format!("node=localhost:{}", other.bitcoind_peer_port),
      "command=add"
    );

    async fn get_number_of_peers(context: &LndTestContext) -> usize {
      let StdoutUntrimmed(json) = cmd!(context.bitcoin_cli_command().await, "getpeerinfo");
      serde_json::from_str::<serde_json::Value>(&json)
        .unwrap()
        .as_array()
        .unwrap()
        .len()
    }
    loop {
      if get_number_of_peers(self).await == 1 && get_number_of_peers(other).await == 1 {
        break;
      }
      tokio::time::sleep(Duration::from_millis(50)).await;
    }
  }

  async fn lnd_pub_key(&self) -> String {
    self.run_lncli_command("getinfo").await["identity_pubkey"]
      .as_str()
      .unwrap()
      .to_string()
  }

  async fn connect_lnds(&self, other: &LndTestContext) {
    self
      .run_lncli_command((
        "connect",
        format!(
          "{}@localhost:{}",
          other.lnd_pub_key().await,
          other.lnd_peer_port
        ),
      ))
      .await;
  }

  pub async fn connect(&self, other: &LndTestContext) {
    self.connect_bitcoinds(other).await;
    self.connect_lnds(other).await;
  }

  pub async fn open_channel_to(&self, other: &LndTestContext, amount: i128) {
    self
      .run_lncli_command((
        "openchannel",
        "--node_key",
        other.lnd_pub_key().await,
        "--local_amt",
        amount.to_string(),
      ))
      .await;
    self.mine_blocks(3).await;
    let payment_request = &other.run_lncli_command("addinvoice").await["payment_request"]
      .as_str()
      .unwrap()
      .to_string();
    loop {
      let (Exit(status), StdoutUntrimmed(_), Stderr(_)) = cmd!(
        self.lncli_command().await,
        "payinvoice",
        "--force",
        "--json",
        "--amt",
        "100",
        payment_request,
      );
      if status.success() {
        break;
      }
      tokio::time::sleep(Duration::from_millis(50)).await;
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use pretty_assertions::assert_eq;

  #[tokio::test]
  async fn starts_lnd() {
    LndTestContext::new().await;
  }

  #[tokio::test]
  async fn generate_money_into_lnd() {
    let context = LndTestContext::new().await;
    let walletbalance = context.run_lncli_command("walletbalance").await;
    assert_eq!(
      walletbalance["total_balance"]
        .as_str()
        .unwrap()
        .parse::<i64>()
        .unwrap(),
      0
    );
    context.generate_money_into_lnd().await;
    let walletbalance = context.run_lncli_command("walletbalance").await;
    let balance = walletbalance["total_balance"]
      .as_str()
      .unwrap()
      .parse::<i64>()
      .unwrap();
    assert!(balance > 0, "{} not greater than 0", balance);
  }

  #[tokio::test]
  async fn connecting_bitcoinds() {
    let a = LndTestContext::new().await;
    let b = LndTestContext::new().await;
    a.connect(&b).await;

    a.mine_blocks(42).await;

    loop {
      let StdoutTrimmed(output) = cmd!(b.bitcoin_cli_command().await, "getblockcount");
      if output == "42" {
        break;
      }
      tokio::time::sleep(Duration::from_millis(50)).await;
    }
  }

  #[tokio::test]
  async fn connecting_lnds() {
    let a = LndTestContext::new().await;
    let b = LndTestContext::new().await;
    a.connect(&b).await;

    assert_eq!(
      a.run_lncli_command("listpeers").await["peers"]
        .as_array()
        .unwrap()
        .len(),
      1
    );
    assert_eq!(
      b.run_lncli_command("listpeers").await["peers"]
        .as_array()
        .unwrap()
        .len(),
      1
    );
  }

  #[tokio::test]
  async fn open_channel() {
    let sender = LndTestContext::new().await;
    let receiver = LndTestContext::new().await;
    sender.connect(&receiver).await;
    sender.generate_money_into_lnd().await;
    sender.open_channel_to(&receiver, 1_000_000).await;

    let payment_request = &receiver.run_lncli_command("addinvoice").await["payment_request"]
      .as_str()
      .unwrap()
      .to_string();
    let status = sender
      .run_lncli_command((
        Split("payinvoice --force --json --amt 10000"),
        payment_request,
      ))
      .await["status"]
      .as_str()
      .unwrap()
      .to_string();
    assert_eq!(status, "SUCCEEDED");
  }
}

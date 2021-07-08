use crate::owned_child::{CommandExt, OwnedChild};
use cradle::*;
use std::{
  fs,
  net::TcpListener,
  path::{Path, PathBuf},
  process::Command,
  thread,
  time::Duration,
};
use tempfile::TempDir;

mod executables;
mod owned_child;

#[derive(Debug)]
pub struct LndTestContext {
  #[allow(unused)]
  bitcoind: OwnedChild,
  bitcoind_rpc_port: u16,
  #[allow(unused)]
  lnd: OwnedChild,
  pub lnd_rpc_port: u16,
  tmpdir: TempDir,
}

impl LndTestContext {
  fn guess_free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
      .unwrap()
      .local_addr()
      .unwrap()
      .port()
  }

  pub async fn new() -> Self {
    let target_dir = executables::target_dir();
    let tmpdir = tempfile::tempdir().unwrap();

    let bitcoinddir = tmpdir.path().join("bitcoind");

    fs::create_dir(&bitcoinddir).unwrap();
    fs::write(bitcoinddir.join("bitcoin.conf"), "\n").unwrap();

    let bitcoind_rpc_port = Self::guess_free_port();
    let zmqpubrawblock = Self::guess_free_port();
    let zmqpubrawtx = Self::guess_free_port();
    let bitcoind = Command::new(executables::bitcoind_executable(&target_dir).await)
      .arg("-chain=regtest")
      .arg(format!("-datadir={}", bitcoinddir.to_str().unwrap()))
      .arg(format!("-rpcport={}", bitcoind_rpc_port))
      .arg("-rpcuser=user")
      .arg("-rpcpassword=password")
      .arg(format!("-port={}", Self::guess_free_port()))
      .arg(format!("-bind=127.0.0.1:{}=onion", Self::guess_free_port()))
      .arg(format!(
        "-zmqpubrawblock=tcp://127.0.0.1:{}",
        zmqpubrawblock
      ))
      .arg(format!("-zmqpubrawtx=tcp://127.0.0.1:{}", zmqpubrawtx))
      .stdout(std::process::Stdio::null())
      .spawn_owned()
      .unwrap();

    let lnddir = tmpdir.path().join("lnd");

    let lnd_rpc_port = Self::guess_free_port();

    let lnd = 'outer: loop {
      let mut lnd = Command::new(executables::lnd_executable(&target_dir).await)
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
        .arg(format!("--listen=127.0.0.1:{}", Self::guess_free_port()))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn_owned()
        .unwrap();
      loop {
        let (Exit(status), Stderr(_), StdoutTrimmed(_)) = cmd!(
          executables::lncli_executable(&executables::target_dir())
            .await
            .to_str()
            .unwrap()
            .to_string(),
          Self::lncli_default_arguments(&lnddir, lnd_rpc_port).await,
          "getinfo"
        );
        if status.success() {
          break 'outer lnd;
        } else if lnd.inner.try_wait().unwrap().is_some() {
          break;
        } else {
          thread::sleep(Duration::from_millis(50));
        }
      }
    };

    Self {
      bitcoind,
      bitcoind_rpc_port,
      lnd,
      lnd_rpc_port,
      tmpdir,
    }
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

  async fn bitcoin_cli_command(&self) -> Vec<String> {
    vec![
      executables::bitcoin_cli_executable(&executables::target_dir())
        .await
        .to_str()
        .unwrap()
        .to_string(),
      "-chain=regtest".to_string(),
      format!("-datadir={}", self.bitcoind_dir()),
      format!("-rpcport={}", self.bitcoind_rpc_port),
      "-rpcuser=user".to_string(),
      "-rpcpassword=password".to_string(),
    ]
  }

  pub async fn lncli_default_arguments(lnd_dir: &Path, lnd_rpc_port: u16) -> Vec<String> {
    vec![
      "--network".to_string(),
      "regtest".to_string(),
      "--lnddir".to_string(),
      lnd_dir.to_str().unwrap().to_string(),
      "--rpcserver".to_string(),
      format!("localhost:{}", lnd_rpc_port),
    ]
  }

  pub async fn lncli_command(&self) -> Vec<String> {
    let mut result = vec![executables::lncli_executable(&executables::target_dir())
      .await
      .to_str()
      .unwrap()
      .to_string()];
    result.extend(Self::lncli_default_arguments(&self.lnd_dir(), self.lnd_rpc_port).await);
    result
  }

  pub async fn run_lncli_command(&self, input: &[&str]) -> serde_json::Value {
    let StdoutUntrimmed(json) = cmd!(self.lncli_command().await, input);
    serde_json::from_str(&json).unwrap()
  }

  async fn generate_bitcoind_wallet_with_money(&self) -> String {
    let StdoutUntrimmed(_) = cmd!(
      self.bitcoin_cli_command().await,
      "createwallet",
      "bitcoin-core-test-wallet"
    );
    let StdoutTrimmed(bitcoind_address) = cmd!(self.bitcoin_cli_command().await, "getnewaddress");
    loop {
      let StdoutUntrimmed(_) = cmd!(self
      .bitcoin_cli_command().await, %"generatetoaddress 10", &bitcoind_address);
      let StdoutTrimmed(balance) = cmd!(self.bitcoin_cli_command().await, "getbalance");
      if balance.parse::<f64>().unwrap() >= 2.0 {
        break;
      }
    }
    bitcoind_address
  }

  pub async fn generate_money_into_lnd(&self) {
    let bitcoind_address = self.generate_bitcoind_wallet_with_money().await;
    let lnd_new_address = self.run_lncli_command(&["newaddress", "p2wkh"]).await["address"]
      .as_str()
      .unwrap()
      .to_string();
    let StdoutUntrimmed(_) = cmd!(
      self.bitcoin_cli_command().await,
      %"-named sendtoaddress amount=1 fee_rate=100",
      format!("address={}", &lnd_new_address),
    );
    loop {
      let StdoutUntrimmed(_) = cmd!(
        self.bitcoin_cli_command().await,
        %"generatetoaddress 1",
        &bitcoind_address
      );
      let walletbalance = self.run_lncli_command(&["walletbalance"]).await;
      let confirmed_balance = &walletbalance["confirmed_balance"]
        .as_str()
        .unwrap()
        .parse::<i64>()
        .unwrap();
      if *confirmed_balance > 0 {
        break;
      }
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
    let walletbalance = context.run_lncli_command(&["walletbalance"]).await;
    assert_eq!(
      walletbalance["total_balance"]
        .as_str()
        .unwrap()
        .parse::<i64>()
        .unwrap(),
      0
    );
    context.generate_money_into_lnd().await;
    let walletbalance = context.run_lncli_command(&["walletbalance"]).await;
    let balance = walletbalance["total_balance"]
      .as_str()
      .unwrap()
      .parse::<i64>()
      .unwrap();
    assert!(balance > 0, "{} not greater than 0", balance);
  }
}

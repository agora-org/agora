use super::*;
use crate::owned_child::{CommandExt, OwnedChild};
use cradle::*;
use hex_literal::hex;
use pretty_assertions::assert_eq;
use sha2::{Digest, Sha256};
use std::{net::TcpListener, path::PathBuf, process::Command, sync::Once};
use tempfile::TempDir;

pub(crate) struct TestContext {
  #[allow(unused)]
  bitcoind: OwnedChild,
  #[allow(unused)]
  lnd: OwnedChild,
  lnd_rest_port: u16,
  tmpdir: TempDir,
}

impl TestContext {
  fn bitcoind_tarball(target_dir: &Path) -> PathBuf {
    const BITCOIN_CORE_TARGET: &str = if cfg!(target_os = "macos") {
      "osx64"
    } else {
      "x86_64-linux-gnu"
    };

    let tarball_path = target_dir.join(format!("bitcoin-0.21.1-{}.tar.gz", BITCOIN_CORE_TARGET));
    if !tarball_path.exists() {
      let mut response = reqwest::blocking::get(format!(
        "https://bitcoin.org/bin/bitcoin-core-0.21.1/bitcoin-0.21.1-{}.tar.gz",
        BITCOIN_CORE_TARGET
      ))
      .unwrap();
      assert_eq!(response.status(), 200);
      let mut tarball_file = std::fs::File::create(&tarball_path).unwrap();
      std::io::copy(&mut response, &mut tarball_file).unwrap();
    }
    tarball_path
  }

  fn bitcoind_executable() -> PathBuf {
    let target_dir = Path::new("../target");
    let binary = target_dir.join("bitcoind");

    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
      if !binary.exists() {
        let tarball_path = Self::bitcoind_tarball(target_dir);
        let tarball_bytes = std::fs::read(&tarball_path).unwrap();
        assert_eq!(
          Sha256::digest(&tarball_bytes).as_slice(),
          if cfg!(target_os = "macos") {
            &hex!("1ea5cedb64318e9868a66d3ab65de14516f9ada53143e460d50af428b5aec3c7")
          } else {
            &hex!("366eb44a7a0aa5bd342deea215ec19a184a11f2ca22220304ebb20b9c8917e2b")
          },
        );
        cmd_unit!(
          %"tar -xzvf",
          tarball_path,
          "-C", target_dir,
          %"--strip-components=2 bitcoin-0.21.1/bin/bitcoin-cli bitcoin-0.21.1/bin/bitcoind"
        );
      }
    });
    binary
  }

  fn lnd_tarball(target_dir: &Path) -> PathBuf {
    let tarball_path = target_dir.join("lnd-source-v0.13.0-beta.tar.gz");
    if !tarball_path.exists() {
      let mut response = reqwest::blocking::get(
        "https://github.com/lightningnetwork/lnd/releases/download/v0.13.0-beta/lnd-source-v0.13.0-beta.tar.gz"
      )
      .unwrap();
      assert_eq!(response.status(), 200);
      let mut tarball_file = std::fs::File::create(&tarball_path).unwrap();
      std::io::copy(&mut response, &mut tarball_file).unwrap();
    }
    tarball_path
  }

  fn lnd_executable() -> PathBuf {
    let target_dir = Path::new("../target");
    let binary = target_dir.join("lnd-itest");
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
      if !binary.exists() {
        let tarball_path = Self::lnd_tarball(target_dir);
        let tarball_bytes = std::fs::read(&tarball_path).unwrap();
        assert_eq!(
          Sha256::digest(&tarball_bytes).as_slice(),
          &hex!("fa8a491dfa40d645e8b6cc4e2b27c5291c0aa0f18de79f2548d0c44e3c2e3912")
        );
        cmd_unit!(
          %"tar -xzvf",
          tarball_path,
          "-C", target_dir
        );
        let src_dir = target_dir.join("lnd-source");
        cmd_unit!(
          %"make build build-itest",
          CurrentDir(&src_dir)
        );
        std::fs::copy(src_dir.join("lncli-debug"), target_dir.join("lncli-debug")).unwrap();
        std::fs::copy(
          src_dir.join("lntest/itest/lnd-itest"),
          target_dir.join("lnd-itest"),
        )
        .unwrap();
      }
    });
    binary
  }

  fn lncli_executable() -> PathBuf {
    Self::lnd_executable().parent().unwrap().join("lncli-debug")
  }

  fn guess_free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
      .unwrap()
      .local_addr()
      .unwrap()
      .port()
  }

  pub(crate) fn new() -> Self {
    let tmpdir = tempfile::tempdir().unwrap();

    let bitcoinddir = tmpdir.path().join("bitcoind");

    std::fs::create_dir(&bitcoinddir).unwrap();
    std::fs::write(bitcoinddir.join("bitcoin.conf"), "\n").unwrap();

    let rpc_port = Self::guess_free_port();
    let zmqpubrawblock = Self::guess_free_port();
    let zmqpubrawtx = Self::guess_free_port();
    let bitcoind = Command::new(Self::bitcoind_executable())
      .arg("-chain=regtest")
      .arg(format!("-datadir={}", bitcoinddir.to_str().unwrap()))
      .arg(format!("-rpcport={}", rpc_port))
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

    let lnd_rest_port = Self::guess_free_port();
    let lnd_rpc_port = Self::guess_free_port();

    let lnd = 'outer: loop {
      let mut lnd = Command::new(Self::lnd_executable())
        .args(&[
          "--bitcoin.regtest",
          "--bitcoin.active",
          "--bitcoin.node=bitcoind",
        ])
        .arg("--lnddir")
        .arg(&lnddir)
        .arg("--bitcoind.dir")
        .arg(&bitcoinddir)
        .arg(format!("--bitcoind.rpchost=127.0.0.1:{}", rpc_port))
        .arg("--bitcoind.rpcuser=user")
        .arg("--bitcoind.rpcpass=password")
        .arg(format!(
          "--bitcoind.zmqpubrawblock=127.0.0.1:{}",
          zmqpubrawblock
        ))
        .arg(format!("--bitcoind.zmqpubrawtx=127.0.0.1:{}", zmqpubrawtx))
        .arg("--noseedbackup")
        .arg("--no-macaroons")
        .arg(format!("--restlisten=127.0.0.1:{}", lnd_rest_port))
        .arg(format!("--rpclisten=127.0.0.1:{}", lnd_rpc_port))
        .arg(format!("--listen=127.0.0.1:{}", Self::guess_free_port()))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn_owned()
        .unwrap();
      loop {
        let (Exit(status), Stderr(_), StdoutTrimmed(_)) = cmd!(
          Self::lncli_executable(),
          "--network",
          "regtest",
          "--lnddir",
          &lnddir,
          "--rpcserver",
          format!("localhost:{}", lnd_rpc_port),
          "--no-macaroons",
          "getinfo"
        );
        if status.success() {
          break 'outer lnd;
        } else if lnd.inner.try_wait().unwrap().is_some() {
          break;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
      }
    };

    Self {
      lnd_rest_port,
      tmpdir,
      lnd,
      bitcoind,
    }
  }

  pub(crate) fn client(&self) -> Client {
    Client::new(
      &self.tmpdir.path().join("lnd/tls.cert"),
      format!("https://localhost:{}", self.lnd_rest_port),
    )
    .unwrap()
    .unwrap()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn starts_lnd() {
    TestContext::new();
  }
}

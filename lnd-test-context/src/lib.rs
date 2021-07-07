use crate::owned_child::{CommandExt, OwnedChild};
use cradle::*;
use hex_literal::hex;
use pretty_assertions::assert_eq;
use sha2::{Digest, Sha256};
use std::{
  env::{self, consts::EXE_SUFFIX},
  fs,
  io::{self, Write},
  net::TcpListener,
  path::{Path, PathBuf},
  process::Command,
  thread,
  time::Duration,
};
use tempfile::TempDir;

mod owned_child;

#[derive(Debug)]
pub struct LndTestContext {
  #[allow(unused)]
  bitcoind: OwnedChild,
  #[allow(unused)]
  lnd: OwnedChild,
  pub lnd_rpc_port: u16,
  tmpdir: TempDir,
}

impl LndTestContext {
  fn target_dir() -> PathBuf {
    let mut dir = env::current_dir().unwrap();

    while !dir.join("target").exists() {
      assert!(dir.pop(), "could not find target directory");
    }

    dir.join("target")
  }

  async fn bitcoind_archive(target_dir: &Path) -> PathBuf {
    const ARCHIVE_SUFFIX: &str = if cfg!(target_os = "macos") {
      "osx64.tar.gz"
    } else if cfg!(target_os = "windows") {
      "win64.zip"
    } else {
      "x86_64-linux-gnu.tar.gz"
    };

    let archive_filename = format!("bitcoin-0.21.1-{}", ARCHIVE_SUFFIX);
    let archive_path = target_dir.join(&archive_filename);
    if !archive_path.exists() {
      let url = format!(
        "https://bitcoin.org/bin/bitcoin-core-0.21.1/{}",
        archive_filename
      );
      #[allow(clippy::explicit_write)]
      writeln!(
        io::stderr(),
        "Downloading Bitcoin Core archive from {}…",
        url
      )
      .unwrap();
      let response = reqwest::get(url).await.unwrap();
      assert_eq!(response.status(), 200);
      let bytes = response.bytes().await.unwrap().to_vec();
      assert_eq!(
        Sha256::digest(&bytes).as_slice(),
        if cfg!(target_os = "macos") {
          &hex!("1ea5cedb64318e9868a66d3ab65de14516f9ada53143e460d50af428b5aec3c7")
        } else if cfg!(target_os = "windows") {
          &hex!("94c80f90184cdc7e7e75988a55b38384de262336abd80b1b30121c6e965dc74e")
        } else {
          &hex!("366eb44a7a0aa5bd342deea215ec19a184a11f2ca22220304ebb20b9c8917e2b")
        },
      );
      let mut archive_file = fs::File::create(&archive_path).unwrap();
      std::io::copy(&mut io::Cursor::new(bytes), &mut archive_file).unwrap();
    }
    archive_path
  }

  async fn bitcoind_executable(target_dir: &Path) -> PathBuf {
    let binary = target_dir.join(format!("bitcoind{}", EXE_SUFFIX));
    static MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _guard = MUTEX.lock().await;
    if !binary.exists() {
      cmd_unit!(
        %"tar -xzvf",
        Self::bitcoind_archive(&target_dir).await,
        "-C", target_dir,
        "--strip-components=2",
        format!("bitcoin-0.21.1/bin/bitcoin-cli{}", EXE_SUFFIX),
        format!("bitcoin-0.21.1/bin/bitcoind{}", EXE_SUFFIX)
      );
    }
    binary
  }

  async fn lnd_tarball(target_dir: &Path) -> PathBuf {
    let tarball_path = target_dir.join("lnd-source-v0.13.0-beta.tar.gz");
    if !tarball_path.exists() {
      let url = "https://github.com/lightningnetwork/lnd/releases/download/v0.13.0-beta/lnd-source-v0.13.0-beta.tar.gz";
      #[allow(clippy::explicit_write)]
      writeln!(io::stderr(), "Downloading LND archive from {}…", url).unwrap();
      let response = reqwest::get(url).await.unwrap();
      assert_eq!(response.status(), 200);
      let bytes = response.bytes().await.unwrap().to_vec();
      assert_eq!(
        Sha256::digest(&bytes).as_slice(),
        &hex!("fa8a491dfa40d645e8b6cc4e2b27c5291c0aa0f18de79f2548d0c44e3c2e3912")
      );
      let mut tarball_file = fs::File::create(&tarball_path).unwrap();
      std::io::copy(&mut io::Cursor::new(bytes), &mut tarball_file).unwrap();
    }
    tarball_path
  }

  async fn lnd_executables(target_dir: &Path) -> (PathBuf, PathBuf) {
    let lnd_itest_filename = format!("lnd-itest{}", EXE_SUFFIX);
    let lncli_debug_filename = format!("lncli-debug{}", EXE_SUFFIX);
    let lnd_itest = target_dir.join(&lnd_itest_filename);
    let lncli_debug = target_dir.join(&lncli_debug_filename);
    static MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _guard = MUTEX.lock().await;
    if !lnd_itest.exists() {
      cmd_unit!(
        %"tar -xzvf",
        Self::lnd_tarball(&target_dir).await,
        "-C", &target_dir
      );
      let src_dir = target_dir.join("lnd-source");
      cmd_unit!(
        %"make build build-itest",
        CurrentDir(&src_dir)
      );
      fs::copy(src_dir.join("lncli-debug"), &lncli_debug).unwrap();
      fs::copy(src_dir.join("lntest/itest/lnd-itest"), &lnd_itest).unwrap();
    }
    (lnd_itest, lncli_debug)
  }

  async fn lnd_executable(target_dir: &Path) -> PathBuf {
    Self::lnd_executables(target_dir).await.0
  }

  async fn lncli_executable(target_dir: &Path) -> PathBuf {
    Self::lnd_executables(target_dir).await.1
  }

  fn guess_free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
      .unwrap()
      .local_addr()
      .unwrap()
      .port()
  }

  pub async fn new() -> Self {
    let target_dir = Self::target_dir();
    let tmpdir = tempfile::tempdir().unwrap();

    let bitcoinddir = tmpdir.path().join("bitcoind");

    fs::create_dir(&bitcoinddir).unwrap();
    fs::write(bitcoinddir.join("bitcoin.conf"), "\n").unwrap();

    let bitcoind_rpc_port = Self::guess_free_port();
    let zmqpubrawblock = Self::guess_free_port();
    let zmqpubrawtx = Self::guess_free_port();
    let bitcoind = Command::new(Self::bitcoind_executable(&target_dir).await)
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
      let mut lnd = Command::new(Self::lnd_executable(&target_dir).await)
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
          Self::lncli_executable(&target_dir).await,
          %"--network regtest",
          "--lnddir",
          &lnddir,
          "--rpcserver",
          format!("localhost:{}", lnd_rpc_port),
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
      lnd,
      lnd_rpc_port,
      tmpdir,
    }
  }

  pub fn lnd_rpc_authority(&self) -> String {
    format!("localhost:{}", self.lnd_rpc_port)
  }

  pub fn lnd_dir(&self) -> PathBuf {
    self.tmpdir.path().join("lnd")
  }

  pub fn invoice_macaroon_path(&self) -> PathBuf {
    self
      .lnd_dir()
      .join("data/chain/bitcoin/regtest/invoice.macaroon")
  }

  pub fn cert_path(&self) -> PathBuf {
    self.lnd_dir().join("tls.cert")
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn starts_lnd() {
    LndTestContext::new().await;
  }
}

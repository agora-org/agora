use cradle::*;
use hex_literal::hex;
use lazy_static::lazy_static;
use pretty_assertions::assert_eq;
use sha2::{Digest, Sha256};
use std::{
  env::{self, consts::EXE_SUFFIX},
  fs,
  io::{self, Write},
  path::{Path, PathBuf},
};

pub fn target_dir() -> &'static Path {
  lazy_static! {
    static ref TARGET_DIR: PathBuf = {
      let mut dir = env::current_dir().unwrap();

      while !dir.join("target").exists() {
        assert!(dir.pop(), "could not find target directory");
      }

      dir.join("target")
    };
  }

  &TARGET_DIR
}

async fn bitcoind_archive() -> PathBuf {
  const ARCHIVE_SUFFIX: &str = if cfg!(target_os = "macos") {
    "osx64.tar.gz"
  } else if cfg!(target_os = "windows") {
    "win64.zip"
  } else {
    "x86_64-linux-gnu.tar.gz"
  };

  let archive_filename = format!("bitcoin-0.21.1-{}", ARCHIVE_SUFFIX);
  let archive_path = target_dir().join(&archive_filename);
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

pub async fn bitcoind() -> PathBuf {
  let binary = target_dir().join(format!("bitcoind{}", EXE_SUFFIX));
  static MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
  let _guard = MUTEX.lock().await;
  if !binary.exists() {
    cmd_unit!(
      %"tar -xzvf",
      bitcoind_archive().await,
      "-C", target_dir(),
      "--strip-components=2",
      format!("bitcoin-0.21.1/bin/bitcoin-cli{}", EXE_SUFFIX),
      format!("bitcoin-0.21.1/bin/bitcoind{}", EXE_SUFFIX)
    );
  }
  binary
}

pub async fn bitcoin_cli() -> PathBuf {
  bitcoind().await.parent().unwrap().join("bitcoin-cli")
}

async fn lnd_tarball() -> PathBuf {
  let tarball_path = target_dir().join("lnd-source-v0.13.0-beta.tar.gz");
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

async fn lnd_executables() -> (PathBuf, PathBuf) {
  let lnd_itest_filename = format!("lnd-itest{}", EXE_SUFFIX);
  let lncli_debug_filename = format!("lncli-debug{}", EXE_SUFFIX);
  let lnd_itest = target_dir().join(&lnd_itest_filename);
  let lncli_debug = target_dir().join(&lncli_debug_filename);
  static MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
  let _guard = MUTEX.lock().await;
  if !lnd_itest.exists() {
    cmd_unit!(
      %"tar -xzvf",
      lnd_tarball().await,
      "-C", target_dir()
    );
    let src_dir = target_dir().join("lnd-source");
    cmd_unit!(
      %"make build build-itest",
      CurrentDir(&src_dir)
    );
    fs::copy(src_dir.join("lncli-debug"), &lncli_debug).unwrap();
    fs::copy(src_dir.join("lntest/itest/lnd-itest"), &lnd_itest).unwrap();
  }
  (lnd_itest, lncli_debug)
}

pub async fn lnd() -> PathBuf {
  lnd_executables().await.0
}

pub async fn lncli() -> PathBuf {
  lnd_executables().await.1
}

use reqwest::{Certificate, Result};
use std::{
  fs::File,
  io::{self, Read},
  path::Path,
};

pub struct Client {
  client: reqwest::blocking::Client,
  base_url: String,
}

impl Client {
  pub fn new(pem_file: &Path, base_url: String) -> io::Result<Result<Client>> {
    let certificate = {
      let mut buf = Vec::new();
      File::open(pem_file)?.read_to_end(&mut buf)?;
      buf
    };
    Ok(Certificate::from_pem(&certificate).and_then(|certificate| {
      reqwest::blocking::Client::builder()
        .add_root_certificate(certificate)
        .build()
        .map(|client| Client { client, base_url })
    }))
  }

  pub fn state(&self) -> Result<String> {
    let response = self.client.execute(
      self
        .client
        .get(format!("{}/v1/state", self.base_url))
        .build()?,
    )?;
    response.error_for_status_ref()?;
    Ok(response.text()?)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use cradle::*;
  use httpmock::{Method::GET, MockServer, Then, When};
  use pretty_assertions::assert_eq;
  use reqwest::StatusCode;
  use std::{
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command},
    sync::Once,
  };
  use tempfile::TempDir;

  fn test<Setup, Test>(setup: Setup, test: Test)
  where
    Setup: FnOnce(When, Then),
    Test: FnOnce(Client),
  {
    let server = MockServer::start();
    server.mock(setup);
    let client = Client::new(Path::new("tests/test-cert.pem"), server.base_url())
      .unwrap()
      .unwrap();
    test(client);
  }

  struct TestContext {
    lnd: OwnedChild,
    bitcoind: OwnedChild,
    tmpdir: TempDir,
  }

  impl TestContext {
    fn new() -> Self {
      let tmpdir = tempfile::tempdir().unwrap();

      let bitcoinddir = tmpdir.path().join("bitcoind");

      std::fs::create_dir(&bitcoinddir).unwrap();
      std::fs::write(bitcoinddir.join("bitcoin.conf"), "\n").unwrap();

      let rpc_port = guess_free_port();
      let zmqpubrawblock = guess_free_port();
      let zmqpubrawtx = guess_free_port();
      let bitcoind = Command::new(bitcoind_executable())
        .arg("-chain=regtest")
        .arg(format!("-datadir={}", bitcoinddir.to_str().unwrap()))
        .arg(format!("-rpcport={}", rpc_port))
        .arg("-rpcuser=user")
        .arg("-rpcpassword=password")
        .arg(format!("-port={}", guess_free_port()))
        .arg(format!("-bind=127.0.0.1:{}=onion", guess_free_port()))
        .arg(format!(
          "-zmqpubrawblock=tcp://127.0.0.1:{}",
          zmqpubrawblock
        ))
        .arg(format!("-zmqpubrawtx=tcp://127.0.0.1:{}", zmqpubrawtx))
        .spawn_owned()
        .unwrap();

      let lnddir = tmpdir.path().join("lnd");

      let lnd = 'outer: loop {
        let mut lnd = Command::new(lnd_executable())
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
          .arg("--debuglevel=trace")
          .spawn_owned()
          .unwrap();
        loop {
          let Exit(status) = cmd!(
            lncli_executable().to_str().unwrap(),
            "--network",
            "regtest",
            "--lnddir",
            lnddir.to_str().unwrap(),
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
        tmpdir,
        lnd,
        bitcoind,
      }
    }

    fn client(&self) -> Client {
      Client::new(
        &self.tmpdir.path().join("lnd/tls.cert"),
        "https://localhost:8080".into(),
      )
      .unwrap()
      .unwrap()
    }
  }

  #[test]
  fn state() {
    assert_eq!(
      TestContext::new().client().state().unwrap(),
      r#"{"state":"RPC_ACTIVE"}"#
    );
  }

  #[test]
  fn returns_error_when_response_status_code_is_not_2xx() {
    test(
      |when, then| {
        when.any_request();
        then.status(404);
      },
      |client| {
        assert_eq!(
          client.state().unwrap_err().status(),
          Some(StatusCode::NOT_FOUND)
        )
      },
    );
  }

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
        let tarball_path = bitcoind_tarball(target_dir);
        cmd_unit!(
          Stdin(
            format!(
              "{}  {}",
              if cfg!(target_os = "macos") {
                "1ea5cedb64318e9868a66d3ab65de14516f9ada53143e460d50af428b5aec3c7"
              } else {
                "366eb44a7a0aa5bd342deea215ec19a184a11f2ca22220304ebb20b9c8917e2b"
              },
              tarball_path.to_str().unwrap(),
            ).as_str()
          ),
          %"shasum -a256 -c -"
        );
        cmd_unit!(
          %"tar -xzvf",
          tarball_path.to_str().unwrap(),
          "-C", target_dir.to_str().unwrap(),
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
        let tarball_path = lnd_tarball(target_dir);
        cmd_unit!(
          Stdin(
            format!(
              "fa8a491dfa40d645e8b6cc4e2b27c5291c0aa0f18de79f2548d0c44e3c2e3912  {}",
              tarball_path.to_str().unwrap(),
            ).as_str()
          ),
          %"shasum -a256 -c -"
        );
        cmd_unit!(
          %"tar -xzvf",
          tarball_path.to_str().unwrap(),
          "-C", target_dir.to_str().unwrap()
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
    lnd_executable().parent().unwrap().join("lncli-debug")
  }

  #[test]
  fn installs_bitcoind_test_executable() {
    let StdoutTrimmed(version) = cmd!(bitcoind_executable().to_str().unwrap(), "--version");
    assert!(version.contains("v0.21.1"));
  }

  #[test]
  fn installs_lnd_test_executable() {
    let StdoutTrimmed(version) = cmd!(lnd_executable().to_str().unwrap(), "--version");
    assert!(version.contains("0.13.0-beta"));
  }

  trait CommandExt {
    fn spawn_owned(&mut self) -> std::io::Result<OwnedChild>;
  }

  struct OwnedChild {
    inner: Child,
  }

  impl CommandExt for Command {
    fn spawn_owned(&mut self) -> std::io::Result<OwnedChild> {
      Ok(OwnedChild {
        inner: self.spawn()?,
      })
    }
  }

  impl Drop for OwnedChild {
    fn drop(&mut self) {
      let _ = self.inner.kill();
      let _ = self.inner.wait();
    }
  }

  fn guess_free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
      .unwrap()
      .local_addr()
      .unwrap()
      .port()
  }

  #[test]
  fn starts_lnd() {
    TestContext::new();
  }
}

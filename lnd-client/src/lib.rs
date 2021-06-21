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
  use std::{path::PathBuf, process::Command, sync::Once};

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

  #[test]
  fn state() {
    test(
      |when, then| {
        when.method(GET).path("/v1/state");
        then.status(200).body("test-state");
      },
      |client| assert_eq!(client.state().unwrap(), "test-state"),
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

  fn lnd_tarball(target_dir: &Path) -> (PathBuf, String) {
    const LND_TARGET: &str = if cfg!(target_os = "macos") {
      "darwin-amd64"
    } else {
      "linux-amd64"
    };
    let tarball_path = target_dir.join(format!("lnd-{}-v0.13.0-beta.tar.gz", LND_TARGET));
    if !tarball_path.exists() {
      let mut response = reqwest::blocking::get(format!(
        "https://github.com/lightningnetwork/lnd/releases/download/v0.13.0-beta/lnd-{}-v0.13.0-beta.tar.gz",
        LND_TARGET
      ))
      .unwrap();
      assert_eq!(response.status(), 200);
      let mut tarball_file = std::fs::File::create(&tarball_path).unwrap();
      std::io::copy(&mut response, &mut tarball_file).unwrap();
    }
    (tarball_path, format!("lnd-{}-v0.13.0-beta", LND_TARGET))
  }

  fn lnd_executable() -> PathBuf {
    let target_dir = Path::new("../target");
    let binary = target_dir.join("lnd");
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
      if !binary.exists() {
        let (tarball_path, tarball_dir) = lnd_tarball(target_dir);
        cmd_unit!(
          Stdin(
            format!(
              "{}  {}",
              if cfg!(target_os = "macos") {
                "be1c3e4a97b54e9265636484590d11c530538b5af273b460e9f154fc0d088c94"
              } else {
                "3aca477c72435876d208a509410a05e7f629bf5e0054c31b9948b56101768347"
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
          "--strip-components=1",
          format!("{}/lnd", tarball_dir),
          format!("{}/lncli", tarball_dir)
        );
      }
    });
    binary
  }

  fn lncli_executable() -> PathBuf {
    lnd_executable().parent().unwrap().join("lncli")
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

  #[test]
  fn starts_lnd() {
    let tmp = tempfile::tempdir().unwrap();

    let bitcoinddir = tmp.path().join("bitcoind");

    std::fs::create_dir(&bitcoinddir).unwrap();

    let bitcoind = Command::new(bitcoind_executable())
      .arg("-chain=regtest")
      .arg(format!("-datadir={}", bitcoinddir.to_str().unwrap()))
      .spawn()
      .unwrap();

    std::thread::sleep(std::time::Duration::from_millis(5000));

    let lnddir = tmp.path().join("lnd");
    let lnd = Command::new(lnd_executable())
      .args(&[
        "--bitcoin.regtest",
        "--bitcoin.active",
        "--bitcoin.node=bitcoind",
      ])
      .arg("--lnddir")
      .arg(&lnddir)
      .arg("--bitcoind.dir")
      .arg(&bitcoinddir)
      .spawn()
      .unwrap();

    cmd_unit!(
      lncli_executable().to_str().unwrap(),
      "--network",
      "regtest",
      "--lnddir",
      lnddir.to_str().unwrap(),
      "getinfo"
    );
  }
}

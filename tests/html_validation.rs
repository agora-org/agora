use std::{io, process::Command, thread::sleep, time::Duration};

#[test]
fn main() -> Result<(), io::Error> {
  let mut server = Command::new("cargo run -- --help").spawn()?;
  sleep(Duration::from_secs(1));
  server.kill()?;
  Ok(())
}

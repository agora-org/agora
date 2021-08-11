use cargo_metadata::MetadataCommand;
use cradle::prelude::*;
use structopt::StructOpt;
use tempfile::tempdir;

#[derive(StructOpt)]
struct Arguments {
  revision: String,
  #[structopt(long)]
  publish_agora_lnd_client: bool,
}

fn main() {
  let arguments = Arguments::from_args();

  let tempdir = tempdir().unwrap();

  (
    "git",
    "clone",
    "git@github.com:agora-org/agora.git",
    CurrentDir(tempdir.path()),
  )
    .run_unit();

  (
    "git",
    "merge-base",
    "--is-ancestor",
    arguments.revision,
    "master",
  )
    .run_unit();

  let metadata = MetadataCommand::new().exec().unwrap();

  let version = metadata
    .packages
    .into_iter()
    .filter(|package| package.name == "agora")
    .next()
    .unwrap()
    .version;

  if arguments.publish_agora_lnd_client {
    (
      "cargo",
      "publish",
      "--dry-run",
      CurrentDir(tempdir.path().join("agora/agora-lnd-client")),
    )
      .run_unit();
  }

  (
    "cargo",
    "publish",
    "--dry-run",
    CurrentDir(tempdir.path().join("agora")),
  )
    .run_unit();

  (
    "git",
    "tag",
    "--sign",
    "--message",
    format!("Release version {}", version),
    version.to_string(),
  )
    .run_unit();

  ("git", "push", &version.to_string()).run_unit();

  if arguments.publish_agora_lnd_client {
    (
      "cargo",
      "publish",
      CurrentDir(tempdir.path().join("agora/agora-lnd-client")),
    )
      .run_unit();
  }

  ("cargo", "publish", CurrentDir(tempdir.path().join("agora"))).run_unit();
}

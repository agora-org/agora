use cargo_metadata::MetadataCommand;
use cradle::prelude::*;
use std::env;
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

  env::set_current_dir(tempdir.path().join("agora")).unwrap();

  (
    "git",
    "merge-base",
    "--is-ancestor",
    &arguments.revision,
    "master",
  )
    .run_unit();

  ("git", "checkout", arguments.revision).run_unit();

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
      CurrentDir("agora-lnd-client"),
    )
      .run_unit();
  }

  ("cargo", "publish", "--dry-run").run_unit();

  (
    "git",
    "tag",
    "--sign",
    "--message",
    format!("Release version {}", version),
    version.to_string(),
  )
    .run_unit();

  ("git", "push", "origin", &version.to_string()).run_unit();

  if arguments.publish_agora_lnd_client {
    ("cargo", "publish", CurrentDir("agora-lnd-client")).run_unit();
  }

  ("cargo", "publish").run_unit();
}

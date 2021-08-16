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
    .run();

  env::set_current_dir(tempdir.path().join("agora")).unwrap();

  (
    "git",
    "merge-base",
    "--is-ancestor",
    &arguments.revision,
    "master",
  )
    .run();

  ("git", "checkout", arguments.revision).run();

  let metadata = MetadataCommand::new().exec().unwrap();

  let version = metadata
    .packages
    .into_iter()
    .filter(|package| package.name == "agora")
    .next()
    .unwrap()
    .version;

  (
    "git",
    "tag",
    "--sign",
    "--message",
    format!("Release version {}", version),
    version.to_string(),
  )
    .run();

  ("git", "push", "origin", &version.to_string()).run();

  if arguments.publish_agora_lnd_client {
    ("cargo", "publish", CurrentDir("agora-lnd-client")).run();
  }

  ("cargo", "publish").run();
}

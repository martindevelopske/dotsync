use std::fmt::Display;

use clap::{Parser, Subcommand};

use crate::config::{CloneArgs, Config, EntryArgs, InitArgs};
mod config;
mod diff;
mod pull;
mod push;
mod sync;

#[derive(Parser, Debug)]
#[command(name="dotsync", version="1.0.0", author= "Martin Ndung'u <martindevelopske@gmail.com" ,about, long_about=None)]
struct Args {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long)]
    config: Option<String>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Sync,
    Diff,
    Push,
    Pull,
    Config,
    Init(InitArgs),
    Add(EntryArgs),
    Clone(CloneArgs),
    Repo,
}

impl Display for Commands {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    println!("this is the command: {}", args.command);

    match args.command {
        Commands::Sync => sync::sync()?,
        Commands::Diff => diff::diff()?,
        Commands::Push => push::push()?,
        Commands::Pull => pull::pull()?,
        Commands::Config => config::config()?,
        Commands::Init(init_args) => Config::init(init_args)?,
        Commands::Add(entry_args) => Config::add_new_entry(entry_args)?,
        Commands::Clone(clone_args) => Config::clone_repo(clone_args)?,
        Commands::Repo => {
            let config = Config::load()?;
            let repo_dir = config.resolve_repo_dir()?;
            println!("resolved repo_dir: {:?}", repo_dir);
        }
    }

    // Config::load().unwrap();

    Ok(())
}

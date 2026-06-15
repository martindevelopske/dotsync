use std::fmt::Display;

use clap::{Parser, Subcommand};

use crate::config::{Config, EntryArgs, InitArgs};
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
    }

    // Config::load().unwrap();

    Ok(())
}

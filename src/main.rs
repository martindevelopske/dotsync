
use std::fmt::Display;

use clap::{Parser, Subcommand};
mod diff;
mod push;
mod pull;
mod sync;


#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
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
    Pull
}

impl Display for Commands{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

fn main() {
    let args = Args::parse();

    println!("this is the command: {}", args.command);

    match args.command{
        Commands::Sync => sync::sync(),
        Commands::Diff=> diff::diff(),
        Commands::Push=> push::push(),
        Commands::Pull=> pull::pull(),
    }
}

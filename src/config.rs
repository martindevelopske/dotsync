use std::{fs, path::PathBuf, vec};

use anyhow::{Context, Ok, Result};
use clap::Args;
use serde::{Deserialize, Serialize};

/// The full dotsync config, mirrored to/from 'config.toml'
#[derive(Deserialize, Serialize, Debug, Default)]
pub struct Config {
    /// Local directory holding the git repo that mirrors your config
    #[serde(default = "Config::default_config_path")]
    repo_dir: PathBuf,

    /// git remote url to push /pull
    #[serde(default)]
    remote: String,

    ///the list of configs being tracked
    #[serde(default)]
    entries: Vec<Entry>,
}

/// a single tracked config: a name plus the path it lives at on disk
#[derive(Deserialize, Serialize, Debug)]
struct Entry {
    /// the sub directory name inside the repo
    name: String,
    /// absolute path to the file or directory on this machine
    source: PathBuf,
}

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Local directory holding the git repo that mirrors your config
    #[arg(long)]
    repo_dir: PathBuf,

    /// git remote url to push /pull
    #[arg(long)]
    remote: String,
    //
    // // ///the list of configs being tracked
    // #[command(flatten)]
    // entries: Vec<Entry>
}
impl InitArgs {
    fn is_empty(&self) -> bool {
        self.repo_dir.as_os_str().is_empty() && self.remote.is_empty()
    }
}
#[derive(Args)]
struct EntryArgs {
    /// the sub directory name inside the repo

    #[arg(short, long)]
    name: String,
    /// absolute path to the file or directory on this machine

    #[arg(short, long)]
    source: PathBuf,
}

impl Config {
    pub fn new(repo_dir: PathBuf, remote: String, entries: Vec<Entry>) -> Self {
        Self {
            repo_dir,
            remote,
            entries,
        }
    }
    pub fn path() -> Result<PathBuf> {
        let dir = dirs::config_dir().context("Could not determine the user config directory")?;
        Ok(dir.join("dotsync").join("config.toml"))
    }
    pub fn exists() -> Result<bool> {
        Ok(Config::path()?.exists())
    }
    fn default_config_path() -> PathBuf {
        dirs::config_dir().unwrap().join("dotsync")
    }
    pub fn load() -> anyhow::Result<Config> {
        let config_path = Config::path()?;

        println!("the  config path is: {:?}", config_path);
        let text = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config at {}", config_path.display()))?;
        let config = toml::from_str(&text)
            .with_context(|| format!("Failed to parse config at {}", config_path.display()))?;
        Ok(config)
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Config::path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
        let text = toml::to_string_pretty(self).context("Failed to serialize config to TOML")?;
        std::fs::write(&path, text)
            .with_context(|| format!("Failed to write config to {}", path.display()))?;
        Ok(())
    }

    pub fn init(init_args: InitArgs) -> Result<()> {
        let config_exists = Config::exists()?;
        if config_exists {
            println!("Configuration already exists. Init not required.");
            return Ok(());
        }
        if init_args.is_empty() {
            println!("Init args are empty: initializing with defaults...");
        }
        println!("the init args provided: {:?}", init_args);
        let config_path = Config::default_config_path().join("config.toml");
        println!("The config path is: {:?}", config_path);
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent).context("Failed to create parent directory")?;
        }
        let new_config = Config::new(init_args.repo_dir, init_args.remote, vec![]);
        let toml =
            toml::to_string_pretty(&new_config).context("Failed to serialize config to toml")?;
        std::fs::write(&config_path, toml).context("Failed to write config to path")?;
        println!("Init Done..");

        return Ok(());
    }
}

pub fn show() -> Result<()> {
    let config = Config::load().with_context(|| format!("Unable to show config "));
    println!("the available config is: {:?}", config);
    Ok(())
}

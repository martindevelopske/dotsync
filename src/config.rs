use std::{fs, path::PathBuf};

use anyhow::{Context, Ok, Result};
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

impl Config {
    pub fn path() -> Result<PathBuf> {
        let dir = dirs::config_dir().context("Could not determine the user config directory")?;
        Ok(dir.join("dotsync").join("config.toml"))
    }
    pub fn exists() -> Result<bool> {
        Ok(Config::path()?.exists())
    }
    fn default_config_path() -> PathBuf {
        dirs::home_dir().unwrap().join("dotsync")
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

    pub fn init() -> Result<()> {
        let config_exists = Config::exists()?;
        if config_exists {
            println!("Configuration already exists. Init not required.");
            Ok(())
        } else {
            let default_config = Config::default();
            println!("config path is: {:?}", default_config.repo_dir);
            let parent = default_config
                .repo_dir
                .parent()
                .context("config path does not contain parent directory")?;
            std::fs::create_dir_all(parent).context("Failed Init: cannot create parent dir")?;
            Ok(())
        }
    }
}

pub fn show() -> Result<()> {
    let config = Config::load().with_context(|| format!("Unable to show config "));
    println!("the available config is: {:?}", config);
    Ok(())
}

use std::{
    fs, io,
    path::PathBuf,
    process::{Command, ExitStatus},
    vec,
};

use anyhow::{Context, Ok, Result};
use clap::Args;
use dialoguer::{Confirm, Input};
use serde::{Deserialize, Serialize};

/// The full dotsync config, mirrored to/from 'config.toml'
#[derive(Deserialize, Serialize, Debug, Default)]
pub struct Config {
    /// Local directory holding the git repo that mirrors your config
    #[serde(default = "Config::default_config_directory")]
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
    fn default_config_directory() -> PathBuf {
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

    fn remote_exists(url: &str) -> bool {
        Command::new("git")
            .args(["ls-remote", url])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    pub fn check_and_set_remote(init_args: &InitArgs) -> Result<()> {
        let directory_to_use = if dirs::config_dir()
            .unwrap()
            .join(&init_args.repo_dir)
            .try_exists()?
        {
            &dirs::config_dir().unwrap().join(&init_args.repo_dir)
        } else {
            &Config::default_config_directory()
        };
        println!("using directory {:?}", &directory_to_use);
        // git check remote
        let output = Command::new("git")
            .args(["remote", "-v"])
            .current_dir(&directory_to_use)
            .output()?;

        let remote_url = String::from_utf8_lossy(&output.stdout).into_owned();

        if !Config::remote_exists(&remote_url) {
            println!(
                "Current set remote url {} does not exist. Setting the new one...",
                &remote_url
            );
        };
        let remote_url = if Config::remote_exists(&remote_url) {
            remote_url
        } else {
            loop {
                let new_url: String = Input::new()
                    .with_prompt("Please provide a valid git url")
                    .interact_text()?;
                if Config::remote_exists(&new_url) {
                    break new_url;
                }
                println!("That URL is not a valid or reachable git repository");
            }
        };
        //set the remote_url
        let exists = std::process::Command::new("git")
            .args(["remote", "get-url", "origin"])
            .current_dir(&directory_to_use)
            .output()?
            .status
            .success();

        if exists {
            Command::new("git")
                .args(["remote", "set-url", "origin", &remote_url])
                .current_dir(&directory_to_use)
                .status()?;
        } else {
            Command::new("git")
                .args(["remote", "add", "origin", &remote_url])
                .current_dir(&directory_to_use)
                .status()?;
        }

        if remote_url.trim().is_empty() {
            // it is a git repo but with a remote not set, set it.
            println!("no remote url configured.");
            let output = Command::new("git")
                .args(["remote", "add", "origin", &init_args.remote])
                .current_dir(&directory_to_use)
                .output()?;
        } else {
            println!("The remote url configured  is: {:?}", remote_url.trim());
        }
        Ok(())
    }

    pub fn init(init_args: InitArgs) -> Result<()> {
        let config_exists = Config::exists()?;
        if config_exists {
            println!("Configuration already exists. Init not required.");
            // return Ok(());
            //
            // check whether parent is a git repo, if not init
        }

        let default_parent_directory = Config::default_config_directory();
        if init_args.is_empty() {
            println!("Init args are empty: initializing with defaults...");
        }
        if dirs::config_dir()
            .unwrap()
            .join(&init_args.repo_dir)
            .try_exists()?
        {
            // check whether parent is a git repo, if not init
            if dirs::config_dir()
                .unwrap()
                .join(&init_args.repo_dir)
                .join(".git")
                .exists()
            {
                println!("already a git repo...")
            } else {
                println!("Not a git repo. Starting initialization...");
                Config::check_and_set_remote(&init_args);
            }
            // ready for push
        } else {
            // do the same but now in the default config directory
            println!(
                "The repo directory {:?} you provided does not exist on your machine, should i use default .config directory?",
                &init_args.repo_dir
            );
            let confirmation = Confirm::new()
                .with_prompt(format!(
                    "Should we continue with default .config directory {:?} [Y/N]",
                    default_parent_directory
                ))
                .interact()
                .unwrap();

            if confirmation {
                Config::check_and_set_remote(&init_args);
            } else {
                // ask for new directory where the config should live - TODO
            };
        }
        //
        // if let Some(parent) = config_path.parent() {
        //     std::fs::create_dir_all(parent).context("Failed to create parent directory")?;
        // }
        // println!("parent does not exist, creating parent directory...");
        // let new_config = Config::new(init_args.repo_dir, init_args.remote, vec![]);
        // let toml =
        //     toml::to_string_pretty(&new_config).context("Failed to serialize config to toml")?;
        // std::fs::write(&config_path, toml).context("Failed to write config to path")?;
        println!("Init Done..");

        return Ok(());
    }
}

pub fn show() -> Result<()> {
    let config = Config::load().with_context(|| format!("Unable to show config "));
    println!("the available config is: {:?}", config);
    Ok(())
}

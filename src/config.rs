use core::panic;
use std::{
    fs::{self, exists},
    path::PathBuf,
    process::Command,
    vec,
};

use anyhow::{Context, Error, Ok, Result, anyhow};
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

impl Entry {
    pub fn new(name: String, source: PathBuf) -> Self {
        Self { name, source }
    }
}

#[derive(Args, Debug)]
pub(crate) struct EntryArgs {
    /// the sub directory name inside the repo
    #[arg(long)]
    name: String,
    /// absolute path to the file or directory on this machine

    #[arg(long)]
    source: PathBuf,
}
#[derive(Args, Debug)]
pub struct InitArgs {
    /// Local directory holding the git repo that mirrors your config
    #[arg(long)]
    repo_dir: PathBuf,

    /// git remote url to push /pull
    #[arg(long)]
    remote: Option<String>,
    //
    // // ///the list of configs being tracked
    // #[command(flatten)]
    // entries: Vec<Entry>
}
impl InitArgs {
    fn is_empty(&self) -> bool {
        self.repo_dir.as_os_str().is_empty() && self.remote.is_none()
    }
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
        std::fs::write(&path, &text)
            .with_context(|| format!("Failed to write config to {}", path.display()))?;

        println!("Saved config: {:?}, to path: {:?}", &text, &path);
        Ok(())
    }
    fn update_config(config: Config) -> Result<()> {
        let mut prev_config = Config::load()?;
        prev_config.repo_dir = config.repo_dir;
        prev_config.remote = config.remote;
        prev_config.entries = config.entries;

        Config::save(&prev_config)?;
        Ok(())
    }

    pub fn add_new_entry(entry: EntryArgs) -> Result<()> {
        let mut prev_config = Config::load()?;

        //check if both paths exist
        let path_exists = fs::exists(&entry.source)?;
        if !path_exists {
            panic!(
                "Invalid path ({:?}) for the given entry. Path does not exist.",
                &entry.source
            );
        }
        let exists = prev_config
            .entries
            .iter()
            .any(|existing| existing.name == entry.name);
        if exists {
            panic!("entry with this name: {} already exists", entry.name);
        }
        prev_config
            .entries
            .append(&mut vec![Entry::new(entry.name, entry.source)]);

        prev_config.save()?;

        Ok(())
    }

    fn remote_exists(url: &str) -> bool {
        Command::new("git")
            .args(["ls-remote", url])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    pub fn check_and_set_remote_origin(init_args: &InitArgs) -> Result<()> {
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
            .args(["remote", "get-url", "origin"])
            .current_dir(&directory_to_use)
            .output()?;

        println!("remote get url output: {:?} ", output);
        let remote_url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        println!("remote.... url {}", &remote_url);
        //
        // if !Config::remote_exists(&remote_url) {
        //     println!(
        //         "Current set remote url {} does not exist. Setting the new one...",
        //         &remote_url
        //     );
        // };
        let final_remote_url = if output.status.success() && Config::remote_exists(&remote_url) {
            println!("remote origin is OK, doing nothing on this.");
            init_args.remote.clone().unwrap_or(remote_url.clone())
        } else {
            println!("Remote exists but is not valid, fixing it...");

            if let Some(remote) = init_args.remote.as_ref() {
                if Config::remote_exists(remote) {
                    if !Config::remote_exists(remote) {
                        return Err(anyhow!("Remote does not exist"));
                    }
                    Command::new("git")
                        .args(["remote", "set-url", "origin", remote])
                        .current_dir(&directory_to_use)
                        .output()?;
                }

                remote.clone()
            } else {
                let new_url = loop {
                    let url: String = Input::new()
                        .with_prompt("Please provide a valid git url")
                        .interact_text()?;

                    if Config::remote_exists(&url) {
                        break url;
                    }

                    println!("That URL is not valid or reachable git repository");
                };

                Command::new("git")
                    .args(["remote", "set-url", "origin", &new_url])
                    .current_dir(directory_to_use)
                    .output()?;

                let new_config = Config::new(init_args.repo_dir.clone(), new_url.clone(), vec![]);
                Config::update_config(new_config)?;

                new_url
            }
        };

        //set the remote_url

        //
        // if rmt_str.lines().any(|r| r.trim()== "origin"){
        //
        //
        // }
        // if exists.stdout(). && Config::remote_exists(rmt.stdout()) {
        //     // no need to set a new one I think, just check if it valid before doing anything else.
        //     println!("Remote url already exists and is valid.");
        //     // Command::new("git")
        //     //     .args(["remote", "set-url", "origin", &remote_url])
        //     //     .current_dir(&directory_to_use)
        //     //     .status()?;
        // } else {
        //     Command::new("git")
        //         .args(["remote", "add", "origin", &remote_url])
        //         .current_dir(&directory_to_use)
        //         .status()?;
        // }

        // if remote_url.trim().is_empty() {
        //     // it is a git repo but with a remote not set, set it.
        //     println!("no remote url configured.");
        //     if let Some(url) = &init_args.remote {
        //         let output = Command::new("git")
        //             .args(["remote", "add", "origin", url])
        //             .current_dir(&directory_to_use)
        //             .output()?;
        //     } else {
        //         //ask for a valid git url
        //         let new_url = loop {
        //             let url: String = Input::new()
        //                 .with_prompt("Please provide a valid git url")
        //                 .interact_text()?;
        //             if Config::remote_exists(&url) {
        //                 break url;
        //             }
        //             println!("That URL is not a valid or reachable git repository");
        //         };
        //         let output = Command::new("git")
        //             .args(["remote", "add", "origin", &new_url])
        //             .current_dir(&directory_to_use)
        //             .output()?;
        //     }
        //     println!("The remote url configured  is: {:?}", remote_url.trim());
        // }
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
        let remote_to_use: String = if let Some(remote) = &init_args.remote {
            println!("repo_dir provided. Initializing with it.");
            remote.clone()
        } else {
            loop {
                let url: String = Input::new()
                    .with_prompt("Please provide a valid git url")
                    .interact_text()?;

                if Config::remote_exists(&url) {
                    break url;
                }

                println!("That URL is not valid or reachable git repository");
            }
        };
        // let repo_dir_to_use = init_args
        //     .repo_dir
        //     .clone()
        //     .unwrap_or_else(Config::default_config_directory);
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
                println!("already a git repo...");
                Config::check_and_set_remote_origin(&init_args)?;
            } else {
                println!("Not a git repo. Starting initialization...");
                Config::check_and_set_remote_origin(&init_args)?;
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
                Config::check_and_set_remote_origin(&init_args)?;
            } else {
                // ask for new directory where the config should live - TODO
            };
        }

        //check if  config file exists, if not create, if present, update.
        if fs::exists(Config::path()?)? {
            let mut current_config = Config::load()?;
            current_config.repo_dir = init_args.repo_dir;
            current_config.remote = remote_to_use;
            current_config.save()?;
        } else {
            let new_config = Config::new(init_args.repo_dir, remote_to_use, vec![]);
            new_config.save()?;
            //create
        }
        // if let Some(parent) = config_path.parent() {
        //     std::fs::create_dir_all(parent).context("Failed to create parent directory")?;
        // }
        // println!("parent does not exist, creating parent directory...");
        // let new_config = Config::new(init_args.repo_dir, init_args.remote, vec![]);
        // let toml =
        //     toml::to_string_pretty(&new_config).context("Failed to serialize config to toml")?;
        // std::fs::write(&config_path, toml).context("Failed to write config to path")?;
        println!("Init Done..");

        Ok(())
    }
}

pub fn config() -> Result<()> {
    let config = Config::load().with_context(|| "Unable to show config ");
    println!("the available config is: {:#?}", config);
    Ok(())
}

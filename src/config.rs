use core::panic;
use std::{
    any,
    fs::{self, exists},
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
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
pub struct CloneArgs {
    /// git remote url to clone from
    #[arg(long)]
    pub remote: String,

    /// local directory to clone into (defaults to ~/dotsync)
    #[arg(long)]
    pub repo_dir: Option<PathBuf>,
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
            anyhow::bail!(
                "Invalid path ({:?}) for the given entry. Path does not exist.",
                &entry.source
            );
        }
        let exists = prev_config
            .entries
            .iter()
            .any(|existing| existing.name == entry.name);
        if exists {
            anyhow::bail!("entry with this name: {} already exists", entry.name);
        }
        prev_config
            .entries
            .append(&mut vec![Entry::new(entry.name, entry.source)]);

        prev_config.save()?;

        Ok(())
    }

    pub fn add_all_entries() -> Result<()> {
        Ok(())
    }

    pub fn push(&self) -> Result<()> {
        let repo_dir = self.resolve_repo_dir()?;
        let mut synced: Vec<&str> = vec![];

        for entry in &self.entries {
            let source = entry
                .source
                .canonicalize()
                .with_context(|| format!("source {:?} does not exist", entry.source))?;

            // source already lives inside repo_dir — no copy needed
            if source.starts_with(&repo_dir) {
                synced.push(&entry.name);
                continue;
            }

            let destination = repo_dir.join(&entry.name);

            if !destination.starts_with(&repo_dir) {
                anyhow::bail!("entry '{}': name escapes repo_dir", entry.name);
            }

            if destination.is_dir() {
                fs::remove_dir_all(&destination)
                    .with_context(|| format!("Failed to remove {:?}", destination))?;
            } else if destination.exists() {
                fs::remove_file(&destination)
                    .with_context(|| format!("Failed to remove {:?}", destination))?;
            }

            copy_recursive(&source, &destination)
                .with_context(|| format!("Failed to copy {:?} to {:?}", source, destination))?;
            synced.push(&entry.name);
        }

        self.write_manifest(&repo_dir)?;

        let porcelain = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&repo_dir)
            .output()?;

        if porcelain.stdout.is_empty() {
            println!("Nothing to commit, repo is up to date.");
            return Ok(());
        }

        Command::new("git")
            .args(["add", "-A"])
            .current_dir(&repo_dir)
            .status()?;

        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Command::new("git")
            .args(["commit", "-m", &format!("sync {}", ts)])
            .current_dir(&repo_dir)
            .status()?;

        if !self.remote.is_empty() {
            Command::new("git")
                .args(["push"])
                .current_dir(&repo_dir)
                .status()?;
        }

        println!("Synced: {:?}", synced);
        Ok(())
    }

    pub fn pull(&self) -> Result<()> {
        let repo_dir = self.resolve_repo_dir()?;

        let status = Command::new("git")
            .args(["pull"])
            .current_dir(&repo_dir)
            .status()?;
        if !status.success() {
            anyhow::bail!("git pull failed");
        }

        let restored = self.restore_from_manifest(&repo_dir)?;
        println!("Restored: {:?}", restored);
        Ok(())
    }

    pub fn clone_repo(args: CloneArgs) -> Result<()> {
        if Config::exists()? {
            anyhow::bail!("a local config already exists — use pull instead");
        }

        let repo_dir = args
            .repo_dir
            .unwrap_or_else(|| dirs::home_dir().unwrap().join("dotsync"));

        if repo_dir.exists() && fs::read_dir(&repo_dir)?.next().is_some() {
            anyhow::bail!("repo_dir {:?} already exists and is non-empty", repo_dir);
        }

        let status = Command::new("git")
            .args(["clone", &args.remote, &repo_dir.to_string_lossy()])
            .status()?;
        if !status.success() {
            anyhow::bail!("git clone failed");
        }

        let repo_dir = repo_dir.canonicalize()?;
        let temp = Config {
            repo_dir: repo_dir.clone(),
            remote: args.remote,
            entries: vec![],
        };

        let restored = temp.restore_from_manifest(&repo_dir)?;
        println!("Cloned and restored: {:?}", restored);
        Ok(())
    }

    pub fn sync(&self) -> Result<()> {
        let repo_dir = self.resolve_repo_dir()?;

        let status = Command::new("git")
            .args(["pull", "--rebase"])
            .current_dir(&repo_dir)
            .status()?;
        if !status.success() {
            anyhow::bail!(
                "git pull --rebase failed — resolve conflicts in {:?} then run push",
                repo_dir
            );
        }

        self.push()
    }

    pub fn diff(&self) -> Result<()> {
        let repo_dir = self.resolve_repo_dir()?;
        let mut any_diff = false;

        for entry in &self.entries {
            let source = entry
                .source
                .canonicalize()
                .with_context(|| format!("source {:?} does not exist", entry.source))?;

            // entries inside repo_dir are covered by git diff below
            if source.starts_with(&repo_dir) {
                continue;
            }

            let destination = repo_dir.join(&entry.name);
            if !destination.exists() {
                println!("--- {} not in repo yet (run push to add it)", entry.name);
                any_diff = true;
                continue;
            }

            let output = Command::new("diff")
                .args(["-ru"])
                .arg(&source)
                .arg(&destination)
                .output()?;

            match output.status.code() {
                Some(0) => {}
                Some(1) => {
                    any_diff = true;
                    print!("{}", String::from_utf8_lossy(&output.stdout));
                }
                _ => anyhow::bail!("diff failed for entry '{}'", entry.name),
            }
        }

        // show uncommitted changes for entries that live inside repo_dir
        let output = Command::new("git")
            .args(["diff", "HEAD"])
            .current_dir(&repo_dir)
            .output()?;
        if !output.stdout.is_empty() {
            any_diff = true;
            print!("{}", String::from_utf8_lossy(&output.stdout));
        }

        if !any_diff {
            println!("No differences — everything is in sync.");
        }

        Ok(())
    }

    fn restore_from_manifest(&self, repo_dir: &Path) -> Result<Vec<String>> {
        let manifest_path = repo_dir.join("dotsync.toml");
        if !manifest_path.exists() {
            anyhow::bail!(
                "no manifest at {:?} — run push first from the source machine",
                manifest_path
            );
        }

        let text = fs::read_to_string(&manifest_path)
            .with_context(|| format!("Failed to read manifest at {:?}", manifest_path))?;
        let mut manifest: Config =
            toml::from_str(&text).context("Failed to parse manifest dotsync.toml")?;

        // expand ~ sources to absolute paths for this machine
        let home = dirs::home_dir().context("Could not determine home directory")?;
        for entry in &mut manifest.entries {
            entry.source = entry
                .source
                .strip_prefix("~")
                .map(|rel| home.join(rel))
                .unwrap_or_else(|_| entry.source.clone());
        }

        // stamp the actual repo_dir so the saved config is correct on this machine
        manifest.repo_dir = repo_dir.to_path_buf();

        let mut restored = vec![];
        for entry in &manifest.entries {
            // source already inside repo_dir — git already updated it
            if entry.source.starts_with(repo_dir) {
                restored.push(entry.name.clone());
                continue;
            }

            let origin = repo_dir.join(&entry.name);
            let origin_real = origin
                .canonicalize()
                .with_context(|| format!("'{}' not found in repo at {:?}", entry.name, origin))?;

            if !origin_real.starts_with(repo_dir) {
                anyhow::bail!("entry '{}': path escapes repo_dir", entry.name);
            }

            if entry.source.is_dir() {
                fs::remove_dir_all(&entry.source)
                    .with_context(|| format!("Failed to remove {:?}", entry.source))?;
            } else if entry.source.exists() {
                fs::remove_file(&entry.source)
                    .with_context(|| format!("Failed to remove {:?}", entry.source))?;
            }
            if let Some(parent) = entry.source.parent() {
                fs::create_dir_all(parent)?;
            }

            copy_recursive(&origin_real, &entry.source)
                .with_context(|| format!("Failed to restore '{}'", entry.name))?;
            restored.push(entry.name.clone());
        }

        manifest.save()?;
        Ok(restored)
    }

    pub fn resolve_repo_dir(&self) -> Result<PathBuf> {
        let via_config_dir = dirs::config_dir()
            .context("Could not determine config dir")?
            .join(&self.repo_dir);
        println!("the repo before canonicalization is: {:?}", via_config_dir);
        if via_config_dir.exists() {
            println!(
                "canonicaliezed version is:{:?} ",
                via_config_dir.canonicalize()?
            );
            return via_config_dir
                .canonicalize()
                .context("Failed to canonicalize repo_dir");
        }
        println!(
            "canonicaliezed version is:{:?} ",
            &self.repo_dir.canonicalize()?
        );
        self.repo_dir
            .canonicalize()
            .with_context(|| format!("repo_dir {:?} does not exist", self.repo_dir))
    }

    fn write_manifest(&self, repo_dir: &Path) -> Result<()> {
        let home = dirs::home_dir().context("Could not determine home directory")?;

        let manifest_entries: Vec<Entry> = self
            .entries
            .iter()
            .map(|e| {
                let rel_source = e
                    .source
                    .strip_prefix(&home)
                    .map(|p| PathBuf::from("~").join(p))
                    .unwrap_or_else(|_| e.source.clone());
                Entry::new(e.name.clone(), rel_source)
            })
            .collect();

        let manifest = Config::new(self.repo_dir.clone(), self.remote.clone(), manifest_entries);
        let text = toml::to_string_pretty(&manifest).context("Failed to serialize manifest")?;
        fs::write(repo_dir.join("dotsync.toml"), text).context("Failed to write manifest")?;

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

fn copy_recursive(src: &Path, dst: &Path) -> Result<()> {
    if src.is_dir() {
        fs::create_dir_all(dst)?;
        for item in fs::read_dir(src)? {
            let item = item?;
            copy_recursive(&item.path(), &dst.join(item.file_name()))?;
        }
    } else {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(src, dst)?;
    }
    Ok(())
}

pub fn config() -> Result<()> {
    let config = Config::load().with_context(|| "Unable to show config ");
    println!("the available config is: {:#?}", config);
    Ok(())
}

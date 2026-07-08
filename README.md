# dotsync

Sync your dotfiles across machines using git.

dotsync tracks your config files in a git repository. On push it copies
your files into the repo, writes a manifest (`dotsync.toml`), and commits.
On pull it reads that manifest to know where each file belongs — so a fresh
machine can clone and restore everything without any manual re-setup.

---

## Installation

```sh
cargo install --path .
```

---

## Setup

### On your primary machine

**1. Initialize**

Point dotsync at an existing directory (or a new one) and a git remote:

```sh
dotsync init --repo-dir .config --remote git@github.com:you/dotfiles.git
```

`--repo-dir` can be an existing directory such as `~/.config`, or a new
dedicated directory. If `--remote` is omitted you will be prompted for a URL.

**2. Track files**

```sh
dotsync add --name nvim   --source ~/.config/nvim
dotsync add --name tmux   --source ~/.tmux.conf
dotsync add --name zsh    --source ~/.zshrc
```

`--name` is the subdirectory name inside the repo. `--source` is the
absolute path to the file or directory on this machine — it must exist.

**3. Push**

```sh
dotsync push
```

### On a fresh machine

```sh
dotsync clone --remote git@github.com:you/dotfiles.git
```

Clone reads the manifest from the repo and restores every file to its
correct location. No re-adding of entries required.

---

## Commands

### `init`

```
dotsync init --repo-dir <path> [--remote <url>]
```

Initializes dotsync for an existing or new directory. If the directory is
not already a git repository, dotsync sets one up. If `--remote` is omitted
you will be prompted interactively.

---

### `add`

```
dotsync add --name <name> --source <path>
```

Registers a file or directory to be tracked. The path must exist on disk.
Duplicate names are rejected.

---

### `push`

```
dotsync push
```

For each tracked entry:

1. Copies the live file or directory from `--source` into `repo_dir/<name>`.
2. Writes `repo_dir/dotsync.toml` (the manifest) with `~`-relative source paths.
3. Runs `git add -A`, commits with a timestamp, and pushes to the remote.

If `source` already lives inside `repo_dir` (e.g. when `repo_dir` is
`~/.config`), no copying is done — the files are already in place and
dotsync skips straight to the git steps.

---

### `pull`

```
dotsync pull
```

Runs `git pull`, then reads `repo_dir/dotsync.toml` to restore each file to
its live location. The manifest — not the local config — is the source of
truth, so pull works correctly on a fresh machine where the local config has
no entries.

---

### `clone`

```
dotsync clone --remote <url> [--repo-dir <path>]
```

Bootstraps a fresh machine in one step:

1. `git clone <remote> <repo_dir>` (defaults to `~/dotsync`).
2. Reads the manifest and restores all files to their live locations.
3. Saves the manifest as the local config so future push and pull work
   immediately.

Refuses to run if a local dotsync config already exists — use `pull` instead.

---

### `sync`

```
dotsync sync
```

The recommended day-to-day command. Runs `git pull --rebase` to absorb
changes pushed from other machines, then runs push to commit and upload your
local changes. Keeps history linear.

If the rebase hits a conflict, dotsync stops with an error. Resolve the
conflict manually inside `repo_dir`, then run `dotsync push`.

---

### `diff`

```
dotsync diff
```

Shows what push would commit. For each tracked entry compares the live file
against its copy in the repo using `diff -ru`. Entries not yet in the repo
are flagged. For entries that live inside `repo_dir`, runs `git diff HEAD`
to show uncommitted changes.

---

### `config`

```
dotsync config
```

Prints the current config loaded from `~/.config/dotsync/config.toml`.

---

## Config file

The local config lives at `~/.config/dotsync/config.toml` and is created
by `init` or `clone`.

```toml
repo_dir = ".config"
remote   = "git@github.com:you/dotfiles.git"

[[entries]]
name   = "nvim"
source = "/home/you/.config/nvim"

[[entries]]
name   = "tmux"
source = "/home/you/.tmux.conf"
```

| Field      | Description                                                      |
| ---------- | ---------------------------------------------------------------- |
| `repo_dir` | Path to the local git repo (relative to `~/.config` or absolute) |
| `remote`   | Git remote URL used for push and pull                            |
| `entries`  | List of tracked files and directories                            |

---

## The manifest

On every push dotsync writes `repo_dir/dotsync.toml` into the repository.
This manifest has the same structure as the local config but stores source
paths relative to `$HOME` (e.g. `~/.config/nvim` instead of
`/home/martin/.config/nvim`), so the same repo works on machines with
different usernames and home paths.

Pull and clone read from the manifest, not from the local config. This is
what makes a fresh-machine restore work: clone the repo, read the manifest,
restore everything.

---

## Two usage patterns

**Dedicated repo** — `repo_dir` is a separate directory (e.g. `~/dotsync`)
that exists only to mirror your dotfiles. Push copies each file in, pull
copies each file out.

**Repo as `.config`** — `repo_dir` is your actual `~/.config`. Your tracked
files already live there, so push skips the copy step and goes straight to
`git add` and commit. Pull similarly just runs `git pull` and restores the
manifest.

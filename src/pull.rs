use anyhow::Result;

use crate::config::Config;

pub fn pull() -> Result<()> {
    let config = Config::load()?;
    config.pull()
}

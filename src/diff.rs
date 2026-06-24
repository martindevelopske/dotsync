use anyhow::Result;

use crate::config::Config;

pub fn diff() -> Result<()> {
    let config = Config::load()?;
    config.diff()
}

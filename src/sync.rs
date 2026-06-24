use anyhow::Result;

use crate::config::Config;

pub fn sync() -> Result<()> {
    let config = Config::load()?;
    config.sync()
}

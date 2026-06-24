use anyhow::Result;

use crate::config::Config;

pub fn push() -> Result<()> {
    let config = Config::load()?;
    config.push()
}

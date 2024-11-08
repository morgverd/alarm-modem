use std::env::var;
use anyhow::{Context, Result};

#[derive(Debug)]
pub(crate) struct Config {
    pub modem_port: String,
    pub modem_baud: u32
}

fn get_env_var(key: &'static str) -> Result<String> {
    var(key).with_context(|| format!("Missing environment variable {}", key))
}

pub(crate) fn from_env() -> Result<Config> {
    Ok(Config {
        modem_port: get_env_var("ALARM_MODEM_PORT")?,
        modem_baud: get_env_var("ALARM_MODEM_BAUD")
            .and_then(|v| v
                .parse::<u32>()
                .context("Failed to parse ALARM_MODEM_PORT into a u32 value")
            )
            .unwrap_or(9600)
    })
}
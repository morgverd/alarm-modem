use std::env::var;
use anyhow::{Context, Result};

pub(crate) struct Config {
    pub modem_port: String,
    pub modem_baud: u32,
    pub webhook_url: String,
    pub webhook_key: String
}

fn get_env_var(key: &'static str) -> Result<String> {
    var(key).with_context(|| format!("Missing environment variable {}", key))
}

pub(crate) fn from_env() -> Result<Config> {
    Ok(Config {
        modem_port: get_env_var("ALARM_MODEM_PORT")?,
        modem_baud: get_env_var("ALARM_MODEM_BAUD")
            .map(|v| v.parse::<u32>().context("Failed to parse ALARM_MODEM_BAUD as u32"))
            .unwrap_or_else(|_| Ok(9600))?,
        webhook_url: get_env_var("ALARM_WEBHOOK_URL")?,
        webhook_key: get_env_var("ALARM_WEBHOOK_KEY")?
    })
}
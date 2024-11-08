mod audio;
mod config;

use std::thread::sleep;
use std::time::{Duration, Instant};
use anyhow::{anyhow, Context, Result};
use dotenv::dotenv;
use env_logger::Env;
use log::{debug, error, info, warn};
use serialport::SerialPort;
use ureq::post;
use crate::audio::listen;
use crate::config::from_env;

const IO_TIMEOUT: Duration = Duration::from_secs(2);
const READ_TIMEOUT: Duration = Duration::from_millis(250);
const READ_EMPTY: Duration = Duration::from_millis(100);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_RETRY_DELAY: Duration = Duration::from_secs(2);
const REQUEST_MAX_RETRIES: i32 = 3;

pub(crate) fn send_webhook(url: &str, key: &str) -> bool {
    let mut attempts = 0;
    while attempts < REQUEST_MAX_RETRIES {

        debug!("Attempting to send webhook request");
        attempts += 1;
        let response = post(url)
            .set("Authorization", key)
            .timeout(REQUEST_TIMEOUT)
            .call();

        match response {
            Ok(resp) => {
                let status = resp.status();
                if status >= 200 && status <= 204 {
                    info!("Successfully sent webhook, got back {status}");
                    return true;
                }
                warn!("Request failed with status: {status}");
            },
            Err(e) => error!("Request failed with error: {}", e)
        }
        sleep(REQUEST_RETRY_DELAY);
    }
    false
}

fn send_command(port: &mut dyn SerialPort, cmd: &'static str) -> Result<String> {
    debug!("Sending command: {}", cmd);
    port.write_all(format!("{}\r", cmd).as_bytes())?;

    let mut buffer = Vec::new();
    let start_time = Instant::now();

    loop {
        if start_time.elapsed() > IO_TIMEOUT {
            debug!("Timeout waiting for response to command: {}", cmd);
            break;
        }

        let bytes_to_read = port.bytes_to_read()? as usize;
        if bytes_to_read > 0 {
            let mut temp_buffer = vec![0; bytes_to_read];
            match port.read(&mut temp_buffer) {
                Ok(n) if n > 0 => {
                    buffer.extend_from_slice(&temp_buffer[..n]);
                    break;
                }
                Ok(_) => {}
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => sleep(READ_TIMEOUT),
                Err(e) => return Err(e.into())
            }
        } else {
            sleep(READ_EMPTY);
        }
    }

    if !buffer.is_empty() {
        let cleaned = String::from_utf8_lossy(&buffer).chars().filter(|c| c.is_alphanumeric()).collect::<String>();
        debug!("Command response: {}", cleaned);
        Ok(cleaned)
    } else {
        Err(anyhow!("Failed to send command"))
    }
}

fn main() -> Result<()> {
    env_logger::init_from_env(Env::new().default_filter_or("info"));

    info!("Loading config");
    dotenv().ok();
    let config = from_env()?;

    debug!("Creating serial port: {} @ {} baud", &config.modem_port, config.modem_baud);
    let mut port = serialport::new(&config.modem_port, config.modem_baud)
        .timeout(IO_TIMEOUT)
        .open()
        .context("Failed to open serial port")?;

    info!("Initializing modem");
    send_command(&mut *port, "ATZ")?;  // Reset
    send_command(&mut *port, "ATE0")?; // Disable echo
    let initialization_commands = vec![
        "AT+FCLASS=8",   // Voice mode
        "AT+VLS=1",      // Enable speaker
        "AT+VGR=3",      // Gain
        "AT+VSM=1,8000", // 8000Hz PCM
        "AT"             // Test connection
    ];
    for cmd in initialization_commands {
        let response = send_command(&mut *port, cmd)?;
        if response != "OK" {
            return Err(anyhow!("Command {cmd} expected OK, instead got: {response}"));
        }
    }

    info!("Connecting to VRX");
    if send_command(&mut *port, "AT+VRX")? != "CONNECT" {
        return Err(anyhow!("Failed to connect to VRX"));
    }

    info!("Listening...");
    listen(&mut *port, || {
        info!("Detected trigger tone, sending webhook");
        if !send_webhook(&config.webhook_url, &config.webhook_key) {
            error!("Failed to send webhook for detection");
        }
    })?;
    Ok(())
}
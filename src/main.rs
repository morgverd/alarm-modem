mod audio;

use std::thread::sleep;
use std::time::{Duration, Instant};
use anyhow::{anyhow, Context, Result};
use env_logger::Env;
use log::{debug, info};
use serialport::SerialPort;
use crate::audio::listen;
// The target tone is average 1665 Hz, 150 power

const MODEM_PORT: &str = "COM3";      // The modem device port
const MODEM_BAUD: u32 = 9600;         // Always 9600 using USB modem
const IO_TIMEOUT: Duration = Duration::from_secs(2);
const READ_TIMEOUT: Duration = Duration::from_millis(250);
const READ_EMPTY: Duration = Duration::from_millis(100);

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
    let mut port = serialport::new(MODEM_PORT, MODEM_BAUD)
        .timeout(IO_TIMEOUT)
        .open()
        .context("Failed to open serial port")?;

    info!("Initializing modem");
    send_command(&mut *port, "ATZ")?; // Reset
    let initialization_commands = vec![
        "ATE0",          // Disable echo
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

    listen(&mut *port, || {
        info!("Triggered!");
    })?;
    Ok(())
}
use std::io::Read;
use std::thread::sleep;
use std::time::{Duration, Instant};
use anyhow::{anyhow, Context, Result};
use serialport::SerialPort;
use rustfft::{FftPlanner, num_complex::Complex};

// The target tone is average 1665 Hz, 150 power

const MODEM_PORT: &str = "COM3";      // The modem device port
const MODEM_BAUD: u32 = 9600;         // Always 9600 using USB modem
const HIGH_PASS_CUTOFF: f32 = 3000.0; // High pass filter cut off
const TONE_MIN_FREQ: f32 = 1640.0;    // Minimum frequency (Hz) for tones
const TONE_MAX_FREQ: f32 = 1720.0;    // Maximum frequency (Hz) for tones
const TONE_MIN_POWER: f32 = 100.0;    // Minimum power for a tone
const TONE_MAX_POWER: f32 = 300.0;    // Maximum power for a tone
const PCM_SAMPLE_RATE: f32 = 8000.0;  // 8000 Hz
const FFT_SAMPLE_SIZE: usize = 1024;  // Buffer size for FFT

const DURATION_IO_TIMEOUT: Duration = Duration::from_secs(2);
const DURATION_CMD_READ_TIMEOUT: Duration = Duration::from_millis(250);
const DURATION_CMD_READ_EMPTY: Duration = Duration::from_millis(100);

fn send_commands(port: &mut dyn SerialPort, commands: Vec<&'static str>) -> Result<()> {
    for cmd in commands {
        println!("Sending command: {}", cmd);
        port.write_all(format!("{}\r", cmd).as_bytes())?;

        let mut buffer = Vec::new();
        let start_time = Instant::now();

        loop {
            if start_time.elapsed() > DURATION_IO_TIMEOUT {
                eprintln!("Timeout waiting for response to command: {}", cmd);
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
                    Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => sleep(DURATION_CMD_READ_TIMEOUT),
                    Err(e) => return Err(e.into())
                }
            } else {
                sleep(DURATION_CMD_READ_EMPTY);
            }
        }

        if !buffer.is_empty() {
            println!("Response: {}", String::from_utf8_lossy(&buffer).chars().filter(|c| !c.is_control()).collect::<String>());
        } else {
            eprintln!("No response received for command: {}", cmd);
        }
    }
    Ok(())
}

fn high_pass_filter(samples: &mut [i16], cutoff: f32) {
    let rc = 1.0 / (cutoff * 2.0 * std::f32::consts::PI);
    let dt = 1.0 / PCM_SAMPLE_RATE;
    let alpha = dt / (rc + dt);

    let mut previous = samples[0] as f32;
    for sample in samples.iter_mut() {
        let filtered = alpha * ((*sample as f32) - previous);
        previous = *sample as f32;
        *sample = filtered as i16;
    }
}

fn calculate_fft(planner: &mut FftPlanner<f32>, samples: &[i16]) -> Vec<Complex<f32>> {
    let fft = planner.plan_fft_forward(samples.len());

    // Convert samples to Complex numbers (Real is sample, Imaginary is 0) & process.
    let mut input: Vec<Complex<f32>> = samples.iter().map(|&s| Complex::new(s as f32, 0.0)).collect();
    fft.process(&mut input);

    input
}

fn detect_tone(fft_output: &[Complex<f32>], sample_rate: f32) -> bool {
    let num_samples = fft_output.len();
    let bin_width = sample_rate / num_samples as f32;

    // Loop over the FFT output and look for frequencies in the modem tone range.
    for (i, &sample) in fft_output.iter().enumerate() {
        let frequency = i as f32 * bin_width;

        // If the frequency is within the tone range, check if the power is above threshold.
        if frequency >= TONE_MIN_FREQ && frequency <= TONE_MAX_FREQ {
            let power = sample.re.powi(2) + sample.im.powi(2);
            if power > TONE_MIN_POWER && power < TONE_MAX_POWER {
                println!("Detected tone at {} Hz with power: {}", frequency, power);
                return true;
            }
        }
    }

    false
}


fn main() -> Result<()> {
    let mut port = serialport::new(MODEM_PORT, MODEM_BAUD)
        .timeout(DURATION_IO_TIMEOUT)
        .open()
        .context("Failed to open serial port")?;

    println!("Initializing...");
    send_commands(&mut *port, vec![
        "ATE0",          // Disable echo
        "ATZ",           // Reset
        "AT",            // Test connection
        "AT+FCLASS=8",   // Voice mode
        "AT+VLS=1",      // Enable Speaker
        "AT+VGR=3",      // Gain
        "AT+VSM=1,8000", // 8000Hz PCM
        "AT+VRX"         // Start receiving
    ])?;

    let mut planner = FftPlanner::<f32>::new();
    planner.plan_fft_forward(FFT_SAMPLE_SIZE);

    println!("Listening...");
    let mut prev_tone_detected = false;
    loop {
        let mut buffer = vec![0; 1024];
        match port.read(&mut buffer) {
            Ok(n) if n > 0 => {

                // Process the samples using FFT.
                let mut samples: Vec<i16> = buffer.iter().map(|&b| b as i16).collect();
                high_pass_filter(&mut samples, HIGH_PASS_CUTOFF);
                let fft_output = calculate_fft(&mut planner, &samples);

                // Check for non-repeated tone triggers.
                let tone_detected = detect_tone(&fft_output, PCM_SAMPLE_RATE);
                if tone_detected && !prev_tone_detected {
                    println!("Tone detected!");
                    prev_tone_detected = true;
                } else if !tone_detected {
                    prev_tone_detected = false;
                }
            }
            Ok(_) => {}
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => sleep(DURATION_CMD_READ_TIMEOUT),
            Err(e) => return Err(anyhow!(e))
        }
    }
}
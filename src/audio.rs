use std::thread::sleep;
use std::time::{Duration, Instant};
use anyhow::anyhow;
use log::{debug, info};
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use serialport::SerialPort;
use anyhow::Result;
use crate::READ_TIMEOUT;

const PCM_SAMPLE_RATE: f32 = 8000.0;  // 8000 Hz
const FFT_SAMPLE_SIZE: usize = 1024;  // Buffer size for FFT
const HIGH_PASS_CUTOFF: f32 = 3000.0; // High pass filter cut off
const TONE_MIN_FREQ: f32 = 1640.0;    // Minimum frequency (Hz) for tones
const TONE_MAX_FREQ: f32 = 1720.0;    // Maximum frequency (Hz) for tones
const TONE_MIN_POWER: f32 = 100.0;    // Minimum power for a tone
const TONE_MAX_POWER: f32 = 300.0;    // Maximum power for a tone
const DETECTION_INTERVAL: Duration = Duration::from_secs(5);

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

fn detect_tone(fft_output: &[Complex<f32>]) -> bool {
    let num_samples = fft_output.len();
    let bin_width = PCM_SAMPLE_RATE / num_samples as f32;

    // Loop over the FFT output and look for frequencies in the modem tone range.
    for (i, &sample) in fft_output.iter().enumerate() {
        let frequency = i as f32 * bin_width;

        // If the frequency is within the tone range, check if the power is above threshold.
        if frequency >= TONE_MIN_FREQ && frequency <= TONE_MAX_FREQ {
            let power = sample.re.powi(2) + sample.im.powi(2);
            if power > TONE_MIN_POWER && power < TONE_MAX_POWER {
                debug!("Detected tone at {} Hz with power: {}", frequency, power);
                return true;
            }
        }
    }

    false
}

pub(crate) fn listen<F>(port: &mut dyn SerialPort, callback: F) -> Result<()>
where
    F: Fn()
{
    let mut planner = FftPlanner::<f32>::new();
    planner.plan_fft_forward(FFT_SAMPLE_SIZE);

    info!("Listening...");
    let mut prev_tone_detected = false;
    let mut prev_time_detected = Instant::now();
    loop {
        let mut buffer = vec![0; 1024];
        match port.read(&mut buffer) {
            Ok(n) if n > 0 => {

                // Process the samples using FFT.
                let mut samples: Vec<i16> = buffer.iter().map(|&b| b as i16).collect();
                high_pass_filter(&mut samples, HIGH_PASS_CUTOFF);
                let fft_output = calculate_fft(&mut planner, &samples);

                // Check for non-repeated tone triggers (exceeding detection interval).
                let tone_detected = detect_tone(&fft_output);
                if tone_detected && !prev_tone_detected {
                    if prev_time_detected.elapsed() >= DETECTION_INTERVAL {
                        debug!("Tone detected!");
                        callback();
                        prev_time_detected = Instant::now();
                    }
                    prev_tone_detected = true;
                } else if !tone_detected {
                    prev_tone_detected = false;
                }
            }
            Ok(_) => {}
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => sleep(READ_TIMEOUT),
            Err(e) => return Err(anyhow!(e))
        }
    }
}
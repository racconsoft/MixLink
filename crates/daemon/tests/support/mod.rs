//! Helpers for the opt-in tests against a real PipeWire.
#![allow(dead_code)]
use std::path::Path;
use std::process::{Child, Command};
use std::time::Duration;

/// 1 kHz stereo sine, 16 bit, 48 kHz.
pub fn write_tone_wav(path: &Path, amplitude: f32, seconds: f32) {
    let rate = 48_000u32;
    let frames = (rate as f32 * seconds) as u32;
    let mut data = Vec::with_capacity(frames as usize * 4);
    for i in 0..frames {
        let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / rate as f32;
        let sample = (amplitude * phase.sin() * i16::MAX as f32) as i16;
        data.extend_from_slice(&sample.to_le_bytes());
        data.extend_from_slice(&sample.to_le_bytes());
    }
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 4).to_le_bytes());
    out.extend_from_slice(&4u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&data);
    std::fs::write(path, out).unwrap();
}

/// Peak absolute sample of a 16-bit PCM or 32-bit float WAV file.
pub fn wav_peak(path: &Path) -> f32 {
    let bytes = std::fs::read(path).unwrap();
    let (mut pos, mut format, mut bits) = (12usize, 1u16, 16u16);
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let body = pos + 8;
        if id == b"fmt " {
            format = u16::from_le_bytes([bytes[body], bytes[body + 1]]);
            bits = u16::from_le_bytes([bytes[body + 14], bytes[body + 15]]);
        }
        if id == b"data" {
            let data = &bytes[body..body.saturating_add(size).min(bytes.len())];
            return match (format, bits) {
                (3 | 0xFFFE, 32) => data
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|c| f32::from_le_bytes(*c).abs())
                    .fold(0.0, f32::max),
                (1, 16) => data
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| (i16::from_le_bytes(*c) as f32 / i16::MAX as f32).abs())
                    .fold(0.0, f32::max),
                _ => panic!("unsupported wav format {format}/{bits}"),
            };
        }
        pos = body + size + (size & 1);
    }
    panic!("no data chunk in {}", path.display());
}

pub fn start_record(path: &Path, target: &str) -> Child {
    Command::new("pw-record")
        .args(["--target", target, "--channels", "2", "--format", "f32"])
        .arg(path)
        .spawn()
        .expect("pw-record")
}

pub fn start_play(path: &Path, target: &str) -> Child {
    Command::new("pw-play")
        .args(["--target", target])
        .arg(path)
        .spawn()
        .expect("pw-play")
}

pub fn stop_record(mut child: Child) {
    std::thread::sleep(Duration::from_millis(300));
    let _ = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status();
    let _ = child.wait();
}

pub fn pw_link(output: &str, input: &str) {
    let status = Command::new("pw-link")
        .args([output, input])
        .status()
        .expect("pw-link");
    assert!(status.success(), "pw-link {output} {input} failed");
}

pub fn pw_unlink(output: &str, input: &str) {
    let status = Command::new("pw-link")
        .args(["-d", output, input])
        .status()
        .expect("pw-link -d");
    assert!(status.success(), "pw-link -d {output} {input} failed");
}

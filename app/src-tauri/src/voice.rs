//! Speech: microphone capture (cpal), speech-to-text (whisper.cpp sidecar),
//! text-to-speech (Windows built-in SAPI via PowerShell).

use crate::tools::{self, s, Tools};
use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use std::path::Path;
use std::process::Child;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

const MAX_SECONDS: usize = 120;

pub struct Recorder {
    stop: Sender<()>,
    buf: Arc<Mutex<Vec<f32>>>,
    rate: u32,
    channels: u16,
    handle: Option<JoinHandle<()>>,
}

fn push<T>(buf: &Mutex<Vec<f32>>, cap: usize, data: &[T], conv: impl Fn(&T) -> f32) {
    let mut b = buf.lock().unwrap();
    if b.len() < cap {
        b.extend(data.iter().map(conv));
    }
}

fn build_stream(buf: Arc<Mutex<Vec<f32>>>) -> Result<(Stream, u32, u16)> {
    let device = cpal::default_host()
        .default_input_device()
        .ok_or_else(|| anyhow!("no microphone found"))?;
    let supported = device.default_input_config()?;
    let format = supported.sample_format();
    let cfg: cpal::StreamConfig = supported.into();
    let (rate, channels) = (cfg.sample_rate.0, cfg.channels);
    let cap = MAX_SECONDS * rate as usize * channels as usize;
    let on_err = |e| eprintln!("audio stream error: {e}");

    let stream = match format {
        SampleFormat::F32 => device.build_input_stream(
            &cfg, move |d: &[f32], _| push(&buf, cap, d, |x| *x), on_err, None)?,
        SampleFormat::I16 => device.build_input_stream(
            &cfg, move |d: &[i16], _| push(&buf, cap, d, |x| *x as f32 / 32768.0), on_err, None)?,
        SampleFormat::U16 => device.build_input_stream(
            &cfg, move |d: &[u16], _| push(&buf, cap, d, |x| (*x as f32 - 32768.0) / 32768.0), on_err, None)?,
        other => return Err(anyhow!("unsupported microphone sample format {other:?}")),
    };
    stream.play()?;
    Ok((stream, rate, channels))
}

impl Recorder {
    /// Start capturing. The cpal stream is !Send, so a dedicated thread owns it.
    pub fn start() -> Result<Self> {
        let buf = Arc::new(Mutex::new(Vec::<f32>::new()));
        let (stop, stop_rx) = mpsc::channel::<()>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(u32, u16), String>>();
        let thread_buf = buf.clone();
        let handle = std::thread::spawn(move || match build_stream(thread_buf) {
            Ok((stream, rate, ch)) => {
                let _ = ready_tx.send(Ok((rate, ch)));
                let _ = stop_rx.recv();
                drop(stream);
            }
            Err(e) => {
                let _ = ready_tx.send(Err(e.to_string()));
            }
        });
        let (rate, channels) = ready_rx
            .recv()
            .map_err(|_| anyhow!("audio thread stopped unexpectedly"))?
            .map_err(|e| anyhow!(e))?;
        Ok(Self { stop, buf, rate, channels, handle: Some(handle) })
    }

    /// Stop and return 16 kHz mono PCM, the format whisper.cpp expects.
    pub fn finish(mut self) -> Vec<i16> {
        let _ = self.stop.send(());
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        let samples = std::mem::take(&mut *self.buf.lock().unwrap());
        to_mono_16k(&samples, self.channels, self.rate)
    }
}

/// Downmix to mono and resample to 16 kHz with linear interpolation.
pub fn to_mono_16k(samples: &[f32], channels: u16, rate: u32) -> Vec<i16> {
    let ch = channels.max(1) as usize;
    let mono: Vec<f32> = samples.chunks(ch).map(|c| c.iter().sum::<f32>() / c.len() as f32).collect();
    if mono.is_empty() || rate == 0 {
        return Vec::new();
    }
    let out_len = (mono.len() as u64 * 16_000 / rate as u64) as usize;
    let last = mono.len() - 1;
    (0..out_len)
        .map(|i| {
            let pos = i as f64 * rate as f64 / 16_000.0;
            let i0 = (pos.floor() as usize).min(last);
            let i1 = (i0 + 1).min(last);
            let frac = (pos - i0 as f64) as f32;
            let v = mono[i0] * (1.0 - frac) + mono[i1] * frac;
            (v.clamp(-1.0, 1.0) * 32767.0) as i16
        })
        .collect()
}

pub fn is_silent(samples: &[i16]) -> bool {
    if samples.len() < 4000 {
        return true; // under 0.25 s
    }
    let sum: f64 = samples.iter().map(|s| (*s as f64).powi(2)).sum();
    (sum / samples.len() as f64).sqrt() < 80.0
}

pub fn write_wav(path: &Path, samples: &[i16]) -> Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec)?;
    for x in samples {
        w.write_sample(*x)?;
    }
    w.finalize()?;
    Ok(())
}

pub async fn transcribe(tools: &Tools, model: &Path, wav: &Path, lang: &str, timestamps: bool) -> Result<String> {
    let mut args = vec!["-m".into(), s(model), "-f".into(), s(wav), "-l".into(), lang.into(), "-np".into()];
    if !timestamps {
        args.push("-nt".into());
    }
    Ok(tools::run(&tools.resolve("whisper-cli"), &args).await?.stdout)
}

/// Drop whisper's non-speech markers such as [BLANK_AUDIO] and join lines.
pub fn clean_transcript(raw: &str) -> String {
    raw.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !(l.starts_with('[') && l.ends_with(']')) && !(l.starts_with('(') && l.ends_with(')')))
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_ts(t: &str) -> Option<f32> {
    let p: Vec<&str> = t.trim().split(':').collect();
    if p.len() != 3 {
        return None;
    }
    Some(p[0].parse::<f32>().ok()? * 3600.0 + p[1].parse::<f32>().ok()? * 60.0 + p[2].parse::<f32>().ok()?)
}

/// Parse `[00:00:01.000 --> 00:00:04.000]  text` lines into (start_seconds, text).
pub fn parse_segments(out: &str) -> Vec<(f32, String)> {
    let mut segs = Vec::new();
    for line in out.lines() {
        let l = line.trim();
        let Some(end) = l.find(']') else { continue };
        if !l.starts_with('[') {
            continue;
        }
        let Some(start) = l[1..end].split("-->").next().and_then(parse_ts) else { continue };
        let text = l[end + 1..].trim();
        if !text.is_empty() {
            segs.push((start, text.to_string()));
        }
    }
    segs
}

pub fn stop_speaking(slot: &Mutex<Option<Child>>) {
    if let Some(mut child) = slot.lock().unwrap().take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// Speak with the Windows voice (SAPI). Text goes through stdin so it is never parsed as a command.
pub fn speak(text: &str, rate: i32, slot: &Mutex<Option<Child>>) -> Result<()> {
    stop_speaking(slot);
    #[cfg(windows)]
    {
        use std::io::Write;
        use std::os::windows::process::CommandExt;
        use std::process::{Command, Stdio};
        let script = format!(
            "[Console]::InputEncoding=[Text.Encoding]::UTF8; Add-Type -AssemblyName System.Speech; \
             $s=New-Object System.Speech.Synthesis.SpeechSynthesizer; $s.Rate={}; \
             $s.Speak([Console]::In.ReadToEnd())",
            rate.clamp(-10, 10)
        );
        let mut child = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .stdin(Stdio::piped())
            .creation_flags(0x0800_0000)
            .spawn()?;
        child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?.write_all(text.as_bytes())?;
        *slot.lock().unwrap() = Some(child);
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (text, rate);
        Err(anyhow!("text-to-speech is only available on Windows"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resamples_48k_stereo_to_16k_mono() {
        let samples = vec![0.5f32; 48_000 * 2]; // 1 s stereo
        let out = to_mono_16k(&samples, 2, 48_000);
        assert_eq!(out.len(), 16_000);
        assert!(out.iter().all(|v| (*v - 16383).abs() <= 1));
    }

    #[test]
    fn parses_whisper_segments() {
        let out = "[00:00:01.500 --> 00:00:04.000]   Hello there\n[00:01:02.000 --> 00:01:03.000]  Bye\nnoise";
        let segs = parse_segments(out);
        assert_eq!(segs, vec![(1.5, "Hello there".to_string()), (62.0, "Bye".to_string())]);
    }

    #[test]
    fn cleans_markers() {
        assert_eq!(clean_transcript("[BLANK_AUDIO]\n hello \n(music)"), "hello");
    }
}

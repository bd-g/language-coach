use std::{
    collections::VecDeque,
    convert::TryFrom,
    path::Path,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use cpal::{
    Sample, SampleFormat, SampleRate, SupportedStreamConfig, SupportedStreamConfigRange,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use language_coach_protocol::{mulaw_to_pcm16, pcm16_to_mulaw};

const PREFERRED_DEVICE_SAMPLE_RATE: u32 = 48_000;
const PREFERRED_DEVICE_CHANNELS: u16 = 1;

/// User-selected audio source for the debug client.
#[derive(Debug, Clone)]
pub enum InputSource {
    Microphone,
    WavFile(std::path::PathBuf),
}

/// Captures local audio and emits 20ms PCMU frames.
pub struct AudioInput {
    mode: InputMode,
}

/// Source of outbound PCMU frames for a debug media session.
pub trait AudioFrameSource {
    /// Produces the next PCMU frame when audio is available.
    fn next_frame(&mut self) -> Option<Vec<u8>>;
}

enum InputMode {
    Microphone {
        queue: Arc<Mutex<Vec<i16>>>,
        _stream: cpal::Stream,
        sample_rate: u32,
        channels: usize,
    },
    Wav {
        samples: Vec<i16>,
        cursor: usize,
        sample_rate: u32,
        channels: usize,
    },
}

impl AudioInput {
    /// Creates a new audio input from the selected source.
    pub fn new(source: &InputSource) -> Result<Self> {
        match source {
            InputSource::Microphone => Self::new_microphone(),
            InputSource::WavFile(path) => Self::new_wav(path),
        }
    }

    /// Produces one 20ms PCMU frame at 8 kHz (160 samples).
    pub fn next_frame(&mut self) -> Option<Vec<u8>> {
        match &mut self.mode {
            InputMode::Microphone {
                queue,
                sample_rate,
                channels,
                ..
            } => {
                let mut guard = queue.lock().ok()?;
                if guard.is_empty() {
                    return None;
                }
                let pcm = take_resampled_frame(&mut guard, *sample_rate, *channels);
                if pcm.is_empty() {
                    return None;
                }
                Some(pcm16_to_mulaw(pcm))
            }
            InputMode::Wav {
                samples,
                cursor,
                sample_rate,
                channels,
            } => {
                if *cursor >= samples.len() {
                    return None;
                }
                let frame_size = usize::try_from(*sample_rate / 50).ok().unwrap_or(160) * *channels;
                let end = (*cursor + frame_size).min(samples.len());
                let chunk = samples.get(*cursor..end)?;
                *cursor = end;
                let mono = downmix_to_mono(chunk, *channels);
                let resampled = resample_i16(&mono, *sample_rate, 8000);
                Some(pcm16_to_mulaw(resampled.into_iter().take(160)))
            }
        }
    }

    fn new_microphone() -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .context("no default input device available")?;
        let config = select_supported_config(device.supported_input_configs()?, "input")?;
        let sample_rate = config.sample_rate().0;
        let channels = usize::from(config.channels()).max(1);
        let queue = Arc::new(Mutex::new(Vec::new()));
        let queue_clone = queue.clone();

        let stream = match config.sample_format() {
            cpal::SampleFormat::I16 => device.build_input_stream(
                &config.clone().into(),
                move |data: &[i16], _| {
                    if let Ok(mut guard) = queue_clone.lock() {
                        guard.extend(data.iter().copied());
                    }
                },
                move |error| {
                    eprintln!("input stream error: {error}");
                },
                None,
            )?,
            cpal::SampleFormat::U16 => device.build_input_stream(
                &config.clone().into(),
                move |data: &[u16], _| {
                    if let Ok(mut guard) = queue_clone.lock() {
                        guard.extend(data.iter().map(|sample| {
                            let shifted = i32::from(*sample) - 32_768;
                            i16::try_from(shifted).unwrap_or_default()
                        }));
                    }
                },
                move |error| {
                    eprintln!("input stream error: {error}");
                },
                None,
            )?,
            cpal::SampleFormat::F32 => device.build_input_stream(
                &config.clone().into(),
                move |data: &[f32], _| {
                    if let Ok(mut guard) = queue_clone.lock() {
                        guard.extend(data.iter().copied().map(f32_to_i16));
                    }
                },
                move |error| {
                    eprintln!("input stream error: {error}");
                },
                None,
            )?,
            other => anyhow::bail!("unsupported input sample format: {other:?}"),
        };

        stream.play()?;
        Ok(Self {
            mode: InputMode::Microphone {
                queue,
                _stream: stream,
                sample_rate,
                channels,
            },
        })
    }

    fn new_wav(path: &Path) -> Result<Self> {
        let reader = hound::WavReader::open(path)
            .with_context(|| format!("failed to open wav file at {}", path.display()))?;
        let spec = reader.spec();
        let sample_rate = spec.sample_rate;
        let channels = usize::from(spec.channels).max(1);
        let samples = reader
            .into_samples::<i16>()
            .collect::<Result<Vec<_>, _>>()
            .context("failed to decode wav samples")?;
        Ok(Self {
            mode: InputMode::Wav {
                samples,
                cursor: 0,
                sample_rate,
                channels,
            },
        })
    }
}

impl AudioFrameSource for AudioInput {
    fn next_frame(&mut self) -> Option<Vec<u8>> {
        Self::next_frame(self)
    }
}

/// Plays received PCMU audio frames on the default speaker.
pub struct AudioOutput {
    queue: Arc<Mutex<VecDeque<i16>>>,
    sample_rate: u32,
    _stream: cpal::Stream,
}

/// Sink for inbound PCMU frames from a debug media session.
pub trait AudioFrameSink {
    /// Accepts a PCMU frame for playback or test inspection.
    fn push_mulaw_frame(&self, payload: &[u8]);
}

impl AudioOutput {
    /// Creates an output stream for playback.
    pub fn new() -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .context("no default output device available")?;
        let config = select_supported_config(device.supported_output_configs()?, "output")?;
        let sample_rate = config.sample_rate().0;
        let channels = usize::from(config.channels()).max(1);
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        let queue_clone = queue.clone();

        let stream = match config.sample_format() {
            cpal::SampleFormat::I16 => device.build_output_stream(
                &config.clone().into(),
                move |out: &mut [i16], _| fill_output_i16(out, channels, &queue_clone),
                move |error| eprintln!("output stream error: {error}"),
                None,
            )?,
            cpal::SampleFormat::U16 => device.build_output_stream(
                &config.clone().into(),
                move |out: &mut [u16], _| fill_output_u16(out, channels, &queue_clone),
                move |error| eprintln!("output stream error: {error}"),
                None,
            )?,
            cpal::SampleFormat::F32 => device.build_output_stream(
                &config.clone().into(),
                move |out: &mut [f32], _| fill_output_f32(out, channels, &queue_clone),
                move |error| eprintln!("output stream error: {error}"),
                None,
            )?,
            other => anyhow::bail!("unsupported output sample format: {other:?}"),
        };
        stream.play()?;
        Ok(Self {
            queue,
            sample_rate,
            _stream: stream,
        })
    }

    /// Queues one PCMU frame for playback.
    pub fn push_mulaw_frame(&self, payload: &[u8]) {
        let decoded = mulaw_to_pcm16(payload.iter().copied());
        let resampled = resample_i16(&decoded, 8000, self.sample_rate);
        if let Ok(mut guard) = self.queue.lock() {
            guard.extend(resampled);
        }
    }
}

impl AudioFrameSink for AudioOutput {
    fn push_mulaw_frame(&self, payload: &[u8]) {
        Self::push_mulaw_frame(self, payload);
    }
}

fn select_supported_config(
    configs: impl IntoIterator<Item = SupportedStreamConfigRange>,
    direction: &str,
) -> Result<SupportedStreamConfig> {
    configs
        .into_iter()
        .map(|range| {
            let sample_rate = preferred_sample_rate(&range);
            let channel_penalty = range.channels().abs_diff(PREFERRED_DEVICE_CHANNELS);
            let rate_penalty = sample_rate.0.abs_diff(PREFERRED_DEVICE_SAMPLE_RATE);
            let format_penalty = sample_format_penalty(range.sample_format());
            (
                channel_penalty,
                rate_penalty,
                format_penalty,
                range.with_sample_rate(sample_rate),
            )
        })
        .min_by_key(|(channel_penalty, rate_penalty, format_penalty, _)| {
            (*channel_penalty, *rate_penalty, *format_penalty)
        })
        .map(|(_, _, _, config)| config)
        .with_context(|| format!("no supported {direction} audio configs available"))
}

fn preferred_sample_rate(range: &SupportedStreamConfigRange) -> SampleRate {
    SampleRate(
        PREFERRED_DEVICE_SAMPLE_RATE.clamp(range.min_sample_rate().0, range.max_sample_rate().0),
    )
}

const fn sample_format_penalty(format: SampleFormat) -> u8 {
    match format {
        SampleFormat::I16 => 0,
        SampleFormat::F32 => 1,
        SampleFormat::U16 => 2,
        _ => 3,
    }
}

fn take_resampled_frame(source: &mut Vec<i16>, in_rate: u32, channels: usize) -> Vec<i16> {
    let desired_in = usize::try_from(in_rate / 50).ok().unwrap_or(160) * channels;
    if source.len() < desired_in.max(1) {
        return Vec::new();
    }
    let chunk = source.drain(0..desired_in).collect::<Vec<_>>();
    let mono = downmix_to_mono(&chunk, channels);
    resample_i16(&mono, in_rate, 8000)
        .into_iter()
        .take(160)
        .collect()
}

fn downmix_to_mono(input: &[i16], channels: usize) -> Vec<i16> {
    let channels = channels.max(1);
    if channels == 1 {
        return input.to_vec();
    }

    input
        .chunks_exact(channels)
        .map(|frame| {
            let sum = frame.iter().map(|sample| i32::from(*sample)).sum::<i32>();
            i16::try_from(sum / i32::try_from(channels).ok().unwrap_or(1)).unwrap_or_default()
        })
        .collect()
}

fn resample_i16(input: &[i16], in_rate: u32, out_rate: u32) -> Vec<i16> {
    if input.is_empty() || in_rate == 0 || out_rate == 0 {
        return Vec::new();
    }
    if in_rate == out_rate {
        return input.to_vec();
    }

    let input_len = u64::try_from(input.len()).ok().unwrap_or(1);
    let output_len_u64 = ((input_len * u64::from(out_rate)) / u64::from(in_rate)).max(1);
    let output_len = usize::try_from(output_len_u64)
        .ok()
        .unwrap_or(input.len().max(1));
    (0..output_len)
        .map(|index| {
            let index_u64 = u64::try_from(index).ok().unwrap_or_default();
            let src_u64 = (index_u64 * u64::from(in_rate)) / u64::from(out_rate);
            let src = usize::try_from(src_u64).ok().unwrap_or_default();
            input
                .get(src.min(input.len() - 1))
                .copied()
                .unwrap_or_default()
        })
        .collect()
}

fn fill_output_i16(out: &mut [i16], channels: usize, queue: &Arc<Mutex<VecDeque<i16>>>) {
    if let Ok(mut guard) = queue.lock() {
        for frame in out.chunks_mut(channels) {
            let value = guard.pop_front().unwrap_or(0);
            frame.fill(value);
        }
    }
}

fn fill_output_u16(out: &mut [u16], channels: usize, queue: &Arc<Mutex<VecDeque<i16>>>) {
    if let Ok(mut guard) = queue.lock() {
        for frame in out.chunks_mut(channels) {
            let value = guard.pop_front().unwrap_or(0);
            let shifted = i32::from(value) + 32_768;
            frame.fill(u16::try_from(shifted).unwrap_or_default());
        }
    }
}

fn fill_output_f32(out: &mut [f32], channels: usize, queue: &Arc<Mutex<VecDeque<i16>>>) {
    if let Ok(mut guard) = queue.lock() {
        for frame in out.chunks_mut(channels) {
            let value = guard.pop_front().unwrap_or(0);
            frame.fill(f32::from(value) / f32::from(i16::MAX));
        }
    }
}

fn f32_to_i16(sample: f32) -> i16 {
    sample.clamp(-1.0, 1.0).to_sample::<i16>()
}

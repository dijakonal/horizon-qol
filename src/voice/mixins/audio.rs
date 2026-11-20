//! The sound card: the microphone feeds 48 kHz mono samples to the mic
//! thread, the speakers pull from the mixer. cpal's streams live on a thread
//! of their own, as they aren't `Send` everywhere.
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, SizedSample, Stream, StreamConfig, SupportedStreamConfig};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Once, mpsc};
use std::time::{Duration, Instant};
use mod_horizon_qol::voice::engine::audio::{AudioBackend, AudioStream, Devices, MicQueue, install_backend};
use mod_horizon_qol::voice::engine::mix::{Mixer, Resampler};
use mod_horizon_qol::voice::engine::opus::SAMPLE_RATE;

type DeviceChoice = (Option<String>, Option<String>);

static BACKEND: CpalBackend = CpalBackend;

pub fn ensure_installed() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| install_backend(&BACKEND));
}

struct CpalBackend;

struct CpalAudio {
    stop: Arc<AtomicBool>,
    restart: mpsc::Sender<DeviceChoice>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl CpalAudio {
    pub fn start(
        mixer: Arc<Mutex<Mixer>>,
        mic: Arc<MicQueue>,
        devices: Arc<Mutex<Devices>>,
        input: Option<String>,
        output: Option<String>,
    ) -> Result<Self, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let (restart, commands) = mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("voice_chat-audio".into())
            .spawn(move || {
                let host = cpal::default_host();
                let mut choice = (input, output);
                let (mut input, mut output, mut active) =
                    open_streams(&host, &choice, &mixer, &mic);
                if !flag.load(Ordering::Relaxed) {
                    *devices.lock().unwrap() = active.clone();
                }
                let mut scanned = Instant::now();
                while !flag.load(Ordering::Relaxed) {
                    let command = commands.recv_timeout(Duration::from_millis(50));
                    if matches!(command, Err(mpsc::RecvTimeoutError::Disconnected)) {
                        break;
                    }
                    let restart = command.is_ok();
                    if let Ok(new) = command {
                        choice = new;
                    }
                    let scan = scanned.elapsed() >= Duration::from_secs(2);
                    let rescan = scan && devices_changed(&host, &choice, &active);
                    if restart || rescan {
                        drop(input);
                        drop(output);
                        mic.clear();
                        (input, output, active) = open_streams(&host, &choice, &mixer, &mic);
                        if !flag.load(Ordering::Relaxed) {
                            *devices.lock().unwrap() = active.clone();
                        }
                    }
                    if restart || scan {
                        scanned = Instant::now();
                    }
                }
                drop(input);
                drop(output);
                mic.clear();
            })
            .map_err(|e| format!("starting audio thread: {e}"))?;
        Ok(Self {
            stop,
            restart,
            worker: Some(worker),
        })
    }

    pub fn restart(&self, input: Option<String>, output: Option<String>) -> Result<(), String> {
        self.restart
            .send((input, output))
            .map_err(|_| "audio thread stopped".into())
    }
}

impl AudioStream for CpalAudio {
    fn restart(&self, input: Option<String>, output: Option<String>) -> Result<(), String> {
        self.restart(input, output)
    }
}

impl AudioBackend for CpalBackend {
    fn start(
        &self,
        mixer: Arc<Mutex<Mixer>>,
        mic: Arc<MicQueue>,
        devices: Arc<Mutex<Devices>>,
        input: Option<String>,
        output: Option<String>,
    ) -> Result<Box<dyn AudioStream>, String> {
        CpalAudio::start(mixer, mic, devices, input, output)
            .map(|audio| Box::new(audio) as Box<dyn AudioStream>)
    }

    fn list_devices(&self) -> (Vec<String>, Vec<String>) {
        list_devices()
    }
}

impl Drop for CpalAudio {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn open_streams(
    host: &cpal::Host,
    choice: &DeviceChoice,
    mixer: &Arc<Mutex<Mixer>>,
    mic: &Arc<MicQueue>,
) -> (Option<Stream>, Option<Stream>, Devices) {
    let (output, out_info) = match pick_device(host, &choice.1, false)
        .ok_or_else(|| "no speakers".into())
        .and_then(|d| open_output(&d, mixer.clone()))
    {
        Ok((stream, name)) => (Some(stream), Ok(name)),
        Err(e) => (None, Err(e)),
    };
    let (input, in_info) = match pick_device(host, &choice.0, true)
        .ok_or_else(|| "no microphone".into())
        .and_then(|d| open_input(&d, mic.clone()))
    {
        Ok((stream, name)) => (Some(stream), Ok(name)),
        Err(e) => (None, Err(e)),
    };
    (
        input,
        output,
        Devices {
            input: in_info,
            output: out_info,
        },
    )
}

fn devices_changed(host: &cpal::Host, choice: &DeviceChoice, active: &Devices) -> bool {
    if active.input.is_err() || active.output.is_err() {
        return true;
    }
    let changed = |requested: &Option<String>, input: bool, current: &Result<String, String>| {
        let names = if input {
            host.input_devices()
                .ok()
                .map(|devices| devices.filter_map(|d| d.name().ok()).collect::<Vec<_>>())
        } else {
            host.output_devices()
                .ok()
                .map(|devices| devices.filter_map(|d| d.name().ok()).collect::<Vec<_>>())
        };
        let Some(names) = names else { return false };
        if let Some(requested) = requested {
            return !names.contains(requested);
        }
        let default = if input {
            host.default_input_device()
        } else {
            host.default_output_device()
        }
        .and_then(|d| d.name().ok());
        current.as_ref().ok() != default.as_ref()
    };
    changed(&choice.0, true, &active.input) || changed(&choice.1, false, &active.output)
}

/// The device's config at 48 kHz when it has one, else its own rate.
fn choose(
    configs: Vec<cpal::SupportedStreamConfigRange>,
    default: Option<SupportedStreamConfig>,
) -> Option<SupportedStreamConfig> {
    let usable = |f: SampleFormat| {
        matches!(
            f,
            SampleFormat::F32 | SampleFormat::I16 | SampleFormat::I32 | SampleFormat::U16
        )
    };
    let at_48k = |c: &cpal::SupportedStreamConfigRange| {
        c.min_sample_rate().0 <= SAMPLE_RATE
            && SAMPLE_RATE <= c.max_sample_rate().0
            && usable(c.sample_format())
    };
    let mut best: Vec<_> = configs.into_iter().filter(at_48k).collect();
    // Fewer channels and float samples first.
    best.sort_by_key(|c| (c.channels(), c.sample_format() != SampleFormat::F32));
    best.into_iter()
        .next()
        .map(|c| c.with_sample_rate(cpal::SampleRate(SAMPLE_RATE)))
        .or(default.filter(|d| usable(d.sample_format())))
}

/// All input and output device names the OS reports, for the settings screen.
/// Sorted and deduplicated, capped at 16 each; empty without a sound card.
pub fn list_devices() -> (Vec<String>, Vec<String>) {
    let host = cpal::default_host();
    let mut inputs = Vec::new();
    if let Ok(devices) = host.input_devices() {
        for d in devices {
            if let Ok(n) = d.name() {
                inputs.push(n);
            }
        }
    }
    let mut outputs = Vec::new();
    if let Ok(devices) = host.output_devices() {
        for d in devices {
            if let Ok(n) = d.name() {
                outputs.push(n);
            }
        }
    }
    let tidy = |mut v: Vec<String>| {
        v.sort();
        v.dedup();
        v.truncate(16);
        v
    };
    (tidy(inputs), tidy(outputs))
}

/// The device with this exact name, or the OS default when it is not found.
fn pick_device(host: &cpal::Host, want: &Option<String>, input: bool) -> Option<cpal::Device> {
    if let Some(name) = want {
        let found = if input {
            host.input_devices().ok().and_then(|mut devices| {
                devices.find(|d| d.name().ok().as_deref() == Some(name.as_str()))
            })
        } else {
            host.output_devices().ok().and_then(|mut devices| {
                devices.find(|d| d.name().ok().as_deref() == Some(name.as_str()))
            })
        };
        if found.is_some() {
            return found;
        }
    }
    if input {
        host.default_input_device()
    } else {
        host.default_output_device()
    }
}

fn open_output(
    device: &cpal::Device,
    mixer: Arc<Mutex<Mixer>>,
) -> Result<(Stream, String), String> {
    let name = device.name().unwrap_or_else(|_| "speakers".into());
    let configs = device
        .supported_output_configs()
        .map(|c| c.collect())
        .unwrap_or_default();
    let supported =
        choose(configs, device.default_output_config().ok()).ok_or("no usable speaker format")?;
    let config: StreamConfig = supported.config();
    let stream = match supported.sample_format() {
        SampleFormat::F32 => output::<f32>(device, &config, mixer, |v| v),
        SampleFormat::I16 => output::<i16>(device, &config, mixer, |v| (v * 32767.0) as i16),
        SampleFormat::I32 => {
            output::<i32>(device, &config, mixer, |v| (v as f64 * 2147483647.0) as i32)
        }
        SampleFormat::U16 => {
            output::<u16>(device, &config, mixer, |v| ((v + 1.0) * 32767.5) as u16)
        }
        _ => return Err("no usable speaker format".into()),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok((stream, name))
}

fn output<T: SizedSample + Send + 'static>(
    device: &cpal::Device,
    config: &StreamConfig,
    mixer: Arc<Mutex<Mixer>>,
    convert: fn(f32) -> T,
) -> Result<Stream, String> {
    let channels = config.channels as usize;
    let rate = config.sample_rate.0;
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                let mut m = mixer.lock().unwrap();
                for frame in data.chunks_mut(channels) {
                    let [l, r] = m.sample(rate);
                    match frame.len() {
                        1 => frame[0] = convert((l + r) * 0.5),
                        _ => {
                            frame[0] = convert(l);
                            frame[1] = convert(r);
                            for s in &mut frame[2..] {
                                *s = convert(0.0);
                            }
                        }
                    }
                }
            },
            |e| eprintln!("voice_chat: speaker error: {e}"),
            None,
        )
        .map_err(|e| e.to_string())
}

fn open_input(device: &cpal::Device, mic: Arc<MicQueue>) -> Result<(Stream, String), String> {
    let name = device.name().unwrap_or_else(|_| "microphone".into());
    let configs = device
        .supported_input_configs()
        .map(|c| c.collect())
        .unwrap_or_default();
    let supported =
        choose(configs, device.default_input_config().ok()).ok_or("no usable microphone format")?;
    let config: StreamConfig = supported.config();
    let stream = match supported.sample_format() {
        SampleFormat::F32 => input::<f32>(device, &config, mic, |v| v),
        SampleFormat::I16 => input::<i16>(device, &config, mic, |v| v as f32 / 32768.0),
        SampleFormat::I32 => {
            input::<i32>(device, &config, mic, |v| (v as f64 / 2147483648.0) as f32)
        }
        SampleFormat::U16 => input::<u16>(device, &config, mic, |v| v as f32 / 32768.0 - 1.0),
        _ => return Err("no usable microphone format".into()),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok((stream, name))
}

fn input<T: SizedSample + Send + 'static>(
    device: &cpal::Device,
    config: &StreamConfig,
    mic: Arc<MicQueue>,
    convert: fn(T) -> f32,
) -> Result<Stream, String> {
    let channels = config.channels as usize;
    let mut resampler = Resampler::new(config.sample_rate.0);
    let mut mono = Vec::new();
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                mono.clear();
                for frame in data.chunks(channels) {
                    let sum: f32 = frame.iter().map(|s| convert(*s)).sum();
                    resampler.push(sum / frame.len() as f32, &mut mono);
                }
                mic.push(&mono);
            },
            |e| eprintln!("voice_chat: microphone error: {e}"),
            None,
        )
        .map_err(|e| e.to_string())
}

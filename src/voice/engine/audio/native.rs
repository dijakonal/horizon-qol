use crate::voice::mix::Mixer;
use crate::voice::opus::SAMPLE_RATE;
use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, OnceLock};

#[derive(Default)]
pub struct MicQueue {
    pub samples: Mutex<VecDeque<f32>>,
    pub ready: Condvar,
}

impl MicQueue {
    pub fn push(&self, samples: &[f32]) {
        let mut q = self.samples.lock().unwrap();
        q.extend(samples);
        let excess = q.len().saturating_sub(SAMPLE_RATE as usize);
        q.drain(..excess);
        drop(q);
        self.ready.notify_one();
    }

    pub fn clear(&self) {
        self.samples.lock().unwrap().clear();
    }
}

#[derive(Clone, Debug)]
pub struct Devices {
    pub input: Result<String, String>,
    pub output: Result<String, String>,
}

impl Default for Devices {
    fn default() -> Self {
        Self {
            input: Err("not open".into()),
            output: Err("not open".into()),
        }
    }
}

pub trait AudioStream: Send {
    fn restart(&self, input: Option<String>, output: Option<String>) -> Result<(), String>;
}

pub trait AudioBackend: Sync {
    fn start(
        &self,
        mixer: Arc<Mutex<Mixer>>,
        mic: Arc<MicQueue>,
        devices: Arc<Mutex<Devices>>,
        input: Option<String>,
        output: Option<String>,
    ) -> Result<Box<dyn AudioStream>, String>;

    fn list_devices(&self) -> (Vec<String>, Vec<String>);
}

static BACKEND: OnceLock<&'static dyn AudioBackend> = OnceLock::new();

#[doc(hidden)]
pub fn install_backend(backend: &'static dyn AudioBackend) {
    let _ = BACKEND.set(backend);
}

pub struct AudioIo {
    inner: Box<dyn AudioStream>,
}

impl AudioIo {
    pub fn start(
        mixer: Arc<Mutex<Mixer>>,
        mic: Arc<MicQueue>,
        devices: Arc<Mutex<Devices>>,
        input: Option<String>,
        output: Option<String>,
    ) -> Result<Self, String> {
        let backend = BACKEND
            .get()
            .ok_or_else(|| "audio backend is unavailable".to_owned())?;
        Ok(Self {
            inner: backend.start(mixer, mic, devices, input, output)?,
        })
    }

    pub fn restart(&self, input: Option<String>, output: Option<String>) -> Result<(), String> {
        self.inner.restart(input, output)
    }
}

pub fn list_devices() -> (Vec<String>, Vec<String>) {
    BACKEND
        .get()
        .map(|backend| backend.list_devices())
        .unwrap_or_default()
}

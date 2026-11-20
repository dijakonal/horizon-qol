use crate::voice::mix::Mixer;
use crate::voice::opus::SAMPLE_RATE;
use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};

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

pub struct AudioIo;

impl AudioIo {
    pub fn start(
        _mixer: Arc<Mutex<Mixer>>,
        _mic: Arc<MicQueue>,
        _devices: Arc<Mutex<Devices>>,
        _input: Option<String>,
        _output: Option<String>,
    ) -> Result<Self, String> {
        Err("audio is unavailable on wasm".into())
    }

    pub fn restart(&self, _input: Option<String>, _output: Option<String>) -> Result<(), String> {
        Err("audio is unavailable on wasm".into())
    }
}

pub fn list_devices() -> (Vec<String>, Vec<String>) {
    Default::default()
}

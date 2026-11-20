//! The voice connection: a UDP socket to the server's voice port, as
//! `ClientVoicechatConnection` runs it (authenticate, then check the
//! connection, then keep it alive), and the mic thread that sends what the
//! player says.
mod worker;

use crate::voice::audio::MicQueue;
use crate::voice::mix::{Listener, Mixer};
use crate::voice::proto::Secret;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Where a server address leads (its SRV record): the game's own resolver.
pub type Redirect = fn(&str, u16) -> (String, u16);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    PushToTalk,
    Voice,
}

/// What the player chose, read by the threads.
#[derive(Clone, Debug)]
pub struct Controls {
    pub mode: Mode,
    pub ptt: bool,
    pub whisper: bool,
    pub muted: bool,
    pub disabled: bool,
    pub threshold_db: f32,
    pub mic_gain: f32,
    pub volume: f32,
    pub player_volumes: HashMap<u128, f32>,
}

#[derive(Clone, Copy)]
struct MicControls {
    mode: Mode,
    ptt: bool,
    whisper: bool,
    muted: bool,
    disabled: bool,
    threshold_db: f32,
    mic_gain: f32,
}

impl Controls {
    fn mic(&self) -> MicControls {
        MicControls {
            mode: self.mode,
            ptt: self.ptt,
            whisper: self.whisper,
            muted: self.muted,
            disabled: self.disabled,
            threshold_db: self.threshold_db,
            mic_gain: self.mic_gain,
        }
    }
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            mode: Mode::PushToTalk,
            ptt: false,
            whisper: false,
            muted: false,
            disabled: false,
            threshold_db: -50.0,
            mic_gain: 1.0,
            volume: 1.0,
            player_volumes: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Status {
    #[default]
    Idle,
    Connecting,
    Connected,
    Failed(String),
}

/// What the game thread and the voice threads share.
pub const IDLE_LEVEL_DB: f32 = -80.0;
const SOURCE_IDLE: Duration = Duration::from_secs(30);

#[derive(Clone, Copy)]
pub struct Activity {
    pub last: Instant,
    pub count: u32,
}

pub struct Shared {
    pub listener: Mutex<Listener>,
    /// Players' eye positions by UUID, this tick.
    pub players: Mutex<HashMap<u128, [f64; 3]>>,
    /// Who was heard last and when, and whether whispering.
    pub talking: Mutex<HashMap<u128, (Instant, bool)>>,
    pub activity: Mutex<HashMap<u128, Activity>>,
    pub controls: Mutex<Controls>,
    pub status: Mutex<Status>,
    pub mic_error: Mutex<Option<String>>,
    /// The microphone's last peak, dB as f32 bits.
    pub mic_level: AtomicU32,
    pub sources: Mutex<HashMap<u128, Instant>>,
    pub transmitting: AtomicBool,
    pub connection: AtomicU64,
}

impl Default for Shared {
    fn default() -> Self {
        Self {
            listener: Mutex::default(),
            players: Mutex::default(),
            talking: Mutex::default(),
            activity: Mutex::default(),
            controls: Mutex::default(),
            status: Mutex::default(),
            mic_error: Mutex::default(),
            mic_level: AtomicU32::new(IDLE_LEVEL_DB.to_bits()),
            sources: Mutex::default(),
            transmitting: AtomicBool::new(false),
            connection: AtomicU64::default(),
        }
    }
}

impl Shared {
    pub fn level(&self) -> f32 {
        f32::from_bits(self.mic_level.load(Ordering::Relaxed))
    }
    fn set_status(&self, s: Status) {
        *self.status.lock().unwrap() = s;
    }

    fn prune(&self, now: Instant) {
        self.talking
            .lock()
            .unwrap()
            .retain(|_, (at, _)| now - *at < Duration::from_secs(2));
        self.sources
            .lock()
            .unwrap()
            .retain(|_, at| now - *at < SOURCE_IDLE);
        self.activity
            .lock()
            .unwrap()
            .retain(|_, activity| now - activity.last < SOURCE_IDLE);
    }
}

pub struct Conn {
    shared: Arc<Shared>,
    generation: u64,
}

impl Conn {
    pub fn start(
        secret: Secret,
        server: (String, u16),
        redirect: Redirect,
        shared: Arc<Shared>,
        mixer: Arc<Mutex<Mixer>>,
        mic: Arc<MicQueue>,
    ) -> Result<Self, String> {
        let generation = shared.connection.fetch_add(1, Ordering::Relaxed) + 1;
        shared.set_status(Status::Connecting);
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("voice_chat-net".into())
            .spawn(move || {
                let result =
                    worker::run(secret, server, redirect, &worker, &mixer, mic, generation);
                if worker.connection.load(Ordering::Relaxed) == generation {
                    mixer.lock().unwrap().clear();
                    worker.transmitting.store(false, Ordering::Relaxed);
                    worker.set_status(match result {
                        Ok(()) => Status::Idle,
                        Err(e) => Status::Failed(e),
                    });
                }
            })
            .map_err(|e| {
                let message = format!("starting network thread: {e}");
                shared.set_status(Status::Failed(message.clone()));
                message
            })?;
        Ok(Self { shared, generation })
    }
}

impl Drop for Conn {
    fn drop(&mut self) {
        if self.shared.connection.load(Ordering::Relaxed) == self.generation {
            self.shared.connection.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_state_expires_independently_of_end_packets() {
        let shared = Shared::default();
        let now = Instant::now();
        let old = now - SOURCE_IDLE - Duration::from_secs(1);
        shared.talking.lock().unwrap().insert(1, (old, false));
        shared.talking.lock().unwrap().insert(2, (now, false));
        shared.sources.lock().unwrap().insert(1, old);
        shared.sources.lock().unwrap().insert(2, now);
        shared.activity.lock().unwrap().insert(
            1,
            Activity {
                last: old,
                count: 1,
            },
        );
        shared.activity.lock().unwrap().insert(
            2,
            Activity {
                last: now,
                count: 1,
            },
        );

        shared.prune(now);

        assert!(!shared.talking.lock().unwrap().contains_key(&1));
        assert!(shared.talking.lock().unwrap().contains_key(&2));
        assert!(!shared.sources.lock().unwrap().contains_key(&1));
        assert!(shared.sources.lock().unwrap().contains_key(&2));
        assert!(!shared.activity.lock().unwrap().contains_key(&1));
        assert!(shared.activity.lock().unwrap().contains_key(&2));
    }
}

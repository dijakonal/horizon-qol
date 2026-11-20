//! Opus, from the vendored pure-Rust `opus-rs`: 48 kHz mono, 20 ms frames,
//! as Simple Voice Chat codes them.

use opus_rs::{Application, OpusDecoder, OpusEncoder};

pub const SAMPLE_RATE: u32 = 48_000;
/// 20 ms at 48 kHz.
pub const FRAME: usize = 960;

// Opus application ids, as Simple Voice Chat's server codec setting names them.
pub const APPLICATION_VOIP: i32 = 2048;
pub const APPLICATION_AUDIO: i32 = 2049;
pub const APPLICATION_RESTRICTED_LOWDELAY: i32 = 2051;

fn application(id: i32) -> Application {
    match id {
        APPLICATION_AUDIO => Application::Audio,
        APPLICATION_RESTRICTED_LOWDELAY => Application::RestrictedLowDelay,
        _ => Application::Voip,
    }
}

pub struct Encoder {
    inner: OpusEncoder,
    app: i32,
    out: Vec<u8>,
}

impl Encoder {
    /// `max_bytes`: the server's MTU, Opus's 1275 at most.
    pub fn new(app: i32, max_bytes: usize) -> Result<Self, String> {
        let inner = OpusEncoder::new(SAMPLE_RATE as i32, 1, application(app))
            .map_err(|e| format!("opus encoder: {e}"))?;
        Ok(Self {
            inner,
            app,
            out: vec![0u8; max_bytes.clamp(64, 1275)],
        })
    }

    pub fn encode(&mut self, frame: &[i16; FRAME]) -> Result<Vec<u8>, String> {
        let n = self
            .inner
            .encode_i16(frame, FRAME, &mut self.out)
            .map_err(|e| format!("opus encode: {e}"))?;
        Ok(self.out[..n].to_vec())
    }

    /// `OPUS_RESET_STATE`: start a fresh stream after a gap in transmission.
    pub fn reset(&mut self) {
        if let Ok(inner) = OpusEncoder::new(SAMPLE_RATE as i32, 1, application(self.app)) {
            self.inner = inner;
        }
    }
}

pub struct Decoder {
    inner: OpusDecoder,
    out: Vec<f32>,
}

impl Decoder {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            inner: OpusDecoder::new(SAMPLE_RATE as i32, 1)
                .map_err(|e| format!("opus decoder: {e}"))?,
            out: vec![0.0; FRAME],
        })
    }

    /// A frame from `packet` as `i16`; None conceals a lost one, and `fec`
    /// rebuilds the frame before `packet` from the redundancy in it. A bad
    /// packet yields silence rather than an error, as the speaker wants a frame
    /// every tick.
    pub fn decode(&mut self, packet: Option<&[u8]>, fec: bool) -> Vec<i16> {
        let decoded = match packet {
            None => self.inner.decode(&[], FRAME, &mut self.out),
            Some(p) if fec => self.inner.decode_fec(p, FRAME, &mut self.out),
            Some(p) => self.inner.decode(p, FRAME, &mut self.out),
        };
        let n = decoded.unwrap_or(0);
        let mut pcm: Vec<i16> = self.out[..n]
            .iter()
            .map(|s| (s * 32768.0).clamp(-32768.0, 32767.0) as i16)
            .collect();
        pcm.resize(FRAME, 0);
        pcm
    }

    pub fn reset(&mut self) {
        if let Ok(inner) = OpusDecoder::new(SAMPLE_RATE as i32, 1) {
            self.inner = inner;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tone_survives_encoding() {
        let mut enc = Encoder::new(APPLICATION_VOIP, 1275).unwrap();
        let mut dec = Decoder::new().unwrap();
        let mut last = Vec::new();
        for f in 0..10 {
            let frame: [i16; FRAME] = std::array::from_fn(|i| {
                let t = (f * FRAME + i) as f32 / SAMPLE_RATE as f32;
                ((t * 440.0 * std::f32::consts::TAU).sin() * 8000.0) as i16
            });
            let packet = enc.encode(&frame).unwrap();
            assert!(!packet.is_empty() && packet.len() <= 1275);
            last = dec.decode(Some(&packet), false);
        }
        let peak = last.iter().map(|s| s.unsigned_abs()).max().unwrap();
        assert!(peak > 2000, "peak {peak}");
        assert_eq!(dec.decode(None, false).len(), FRAME);
    }

    #[test]
    fn reset_keeps_working() {
        let mut enc = Encoder::new(APPLICATION_VOIP, 512).unwrap();
        enc.reset();
        let frame = [0i16; FRAME];
        assert!(enc.encode(&frame).is_ok());
    }
}

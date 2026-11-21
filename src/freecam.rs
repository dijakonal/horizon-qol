//! Freecam: the camera leaves the player and flies on its own, through blocks, while the player
//! stands still where they were.
//!
//! The game keeps the player's look and movement; the mod takes them over while Freecam is on.
//! Each tick the movement keys move the camera instead of the player (their input to the player
//! is zeroed), and the mouse's turn of the player is moved onto the camera and the player's look
//! put back. Each frame the camera is drawn between its last two ticks' places, turned by the
//! mouse's latest move too, so it moves as smoothly as the player's own view.

/// The camera while Freecam is on.
#[derive(Clone, Debug, PartialEq)]
pub struct Freecam {
    /// Where the camera is now, and was at the tick before.
    pub pos: [f64; 3],
    pub prev: [f64; 3],
    /// Where it looks, in degrees: yaw as the player's (0 faces south, +Z), pitch -90 up.
    pub yaw: f32,
    pub pitch: f32,
    /// The player's look when Freecam came on, kept on them while it is on.
    pub frozen: [f32; 2],
    /// The movement keys this tick: forward (+1) or back, left (+1) or right, up (+1) or down.
    pub input: [f32; 3],
    /// The sprint key: twice as fast.
    pub fast: bool,
}

impl Freecam {
    /// A camera at the player's eye `eye`, looking as they look (`look`).
    pub fn new(eye: [f64; 3], look: [f32; 2]) -> Self {
        Self {
            pos: eye,
            prev: eye,
            yaw: look[0],
            pitch: look[1],
            frozen: look,
            input: [0.0; 3],
            fast: false,
        }
    }

    /// The player's look is `now`: what the mouse turned them since the last tick goes to the
    /// camera. The mod then puts the player's look back to `frozen`.
    pub fn turn(&mut self, now: [f32; 2]) {
        let [yaw, pitch] = self.look(now);
        self.yaw = yaw;
        self.pitch = pitch;
    }

    /// Where the camera looks with the player's look at `now`, before the next tick takes it.
    pub fn look(&self, now: [f32; 2]) -> [f32; 2] {
        [
            self.yaw + (now[0] - self.frozen[0]),
            (self.pitch + (now[1] - self.frozen[1])).clamp(-90.0, 90.0),
        ]
    }

    /// One tick's move at `speed` blocks a second (twice that with sprint held): along the
    /// ground the way it faces, or, with `free`, straight where it looks.
    pub fn step(&mut self, speed: f32, free: bool) {
        self.prev = self.pos;
        let [forward, left, up] = self.input;
        let yaw = (self.yaw as f64).to_radians();
        let pitch = (self.pitch as f64).to_radians();
        // Facing: yaw 0 looks along +Z, 90 along -X.
        let (fx, fz) = (-yaw.sin(), yaw.cos());
        let (lx, lz) = (fz, -fx);
        let mut d = if free {
            [
                fx * pitch.cos() * forward as f64 + lx * left as f64,
                -pitch.sin() * forward as f64 + up as f64,
                fz * pitch.cos() * forward as f64 + lz * left as f64,
            ]
        } else {
            [
                fx * forward as f64 + lx * left as f64,
                up as f64,
                fz * forward as f64 + lz * left as f64,
            ]
        };
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        if len < 1e-6 {
            return;
        }
        let per_tick = speed as f64 / 20.0 * if self.fast { 2.0 } else { 1.0 };
        for v in &mut d {
            *v *= per_tick / len;
        }
        for (p, v) in self.pos.iter_mut().zip(d) {
            *p += v;
        }
    }

    /// Where the camera is drawn `partial` of the way into the tick.
    pub fn at(&self, partial: f32) -> [f64; 3] {
        let t = partial.clamp(0.0, 1.0) as f64;
        [0, 1, 2].map(|i| self.prev[i] + (self.pos[i] - self.prev[i]) * t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: [f64; 3], b: [f64; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6)
    }

    #[test]
    fn it_flies_the_way_it_faces_and_turns_with_the_mouse() {
        let mut c = Freecam::new([0.0, 64.0, 0.0], [0.0, 0.0]);
        // Facing south (+Z), forward at 10 blocks a second: half a block a tick.
        c.input = [1.0, 0.0, 0.0];
        c.step(10.0, false);
        assert!(near(c.pos, [0.0, 64.0, 0.5]));
        assert!(near(c.at(0.5), [0.0, 64.0, 0.25]));
        // The mouse turned the player 90 degrees right (to the west, -X): the camera turns.
        c.turn([90.0, 0.0]);
        assert_eq!(c.yaw, 90.0);
        c.step(10.0, false);
        assert!(near(c.pos, [-0.5, 64.0, 0.5]));
        // Left of west is south; up and sprint, diagonally, still at its speed.
        c.input = [0.0, 1.0, 1.0];
        c.fast = true;
        c.step(10.0, false);
        let d = 1.0 / 2f64.sqrt();
        assert!(near(c.pos, [-0.5, 64.0 + d, 0.5 + d]));
        // Looking straight down, forward goes down only where it looks.
        c.input = [1.0, 0.0, 0.0];
        c.fast = false;
        c.pitch = 90.0;
        let before = c.pos;
        c.step(10.0, true);
        assert!(near(c.pos, [before[0], before[1] - 0.5, before[2]]));
        // Pitch never goes past straight up or down.
        assert_eq!(c.look([90.0, 50.0])[1], 90.0);
    }
}

//! Presentation-only motion. The renderer's dimensions never follow this clock.
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub scale: f32,
    pub x: f32,
    pub y: f32,
}
impl Default for Pose {
    fn default() -> Self {
        Self {
            scale: 1.,
            x: 0.,
            y: 0.,
        }
    }
}
#[derive(Clone, Copy, Default)]
struct Spring {
    value: f32,
    velocity: f32,
}
impl Spring {
    fn step(&mut self, target: f32, dt: f32) {
        // Exact critically damped solution; frame delays cannot destabilize it.
        let omega = 30.;
        let offset = self.value - target;
        let carry = self.velocity + omega * offset;
        let decay = (-omega * dt).exp();
        self.value = target + (offset + carry * dt) * decay;
        self.velocity = (self.velocity - omega * carry * dt) * decay;
    }
}
pub struct Motion {
    scale: Spring,
    x: Spring,
    y: Spring,
    target: Pose,
    released: Option<(f32, Pose)>,
    painted: Option<f32>,
    pub reduced: bool,
}
pub struct Sample {
    pub pose: Pose,
    pub blend: f32,
    pub finished: bool,
}
pub const PAINT_DELAY: f32 = 0.032;
pub const PAINT_LIMIT: f32 = 0.240;
fn ease_out(t: f32) -> f32 {
    1. - (1. - t.clamp(0., 1.)).powi(3)
}
fn smooth(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    t * t * (3. - 2. * t)
}
impl Motion {
    pub fn new(reduced: bool) -> Self {
        Self {
            scale: Spring {
                value: 1.,
                velocity: 0.,
            },
            x: Spring::default(),
            y: Spring::default(),
            target: Pose::default(),
            released: None,
            painted: None,
            reduced,
        }
    }
    pub fn dragging(&self) -> bool {
        self.released.is_none()
    }
    pub fn resume(&mut self, pose: Pose) {
        self.scale.value = pose.scale;
        self.x.value = pose.x;
        self.y.value = pose.y;
    }
    /// A newly shown split pane starts lifted beside the divider, then uses
    /// the same paint handoff and nonlinear settle as a released resize.
    pub fn enter(&mut self, edge: f32) {
        self.point(edge, 0.);
        self.resume(self.target);
        self.release(0.);
    }
    pub fn point(&mut self, x: f32, y: f32) {
        self.target = if self.reduced {
            Pose::default()
        } else {
            Pose {
                scale: 0.968,
                x: -x.clamp(-1., 1.) * 0.13,
                y: -y.clamp(-1., 1.) * 0.045,
            }
        };
    }
    pub fn release(&mut self, now: f32) {
        self.released = Some((now, self.pose()));
        self.painted = None;
    }
    pub fn paint_ready(&mut self, now: f32) {
        if self.released.is_some() && self.painted.is_none() {
            self.painted = Some(now);
        }
    }
    pub fn elapsed(&self, now: f32) -> Option<f32> {
        self.released.map(|(start, _)| now - start)
    }
    fn pose(&self) -> Pose {
        Pose {
            scale: self.scale.value,
            x: self.x.value,
            y: self.y.value,
        }
    }
    pub fn sample(&mut self, now: f32, dt: f32) -> Sample {
        let Some((start, from)) = self.released else {
            self.scale.step(self.target.scale, dt);
            self.x.step(self.target.x, dt);
            self.y.step(self.target.y, dt);
            return Sample {
                pose: self.pose(),
                blend: 0.,
                finished: false,
            };
        };
        let elapsed = (now - start).max(0.);
        let fade_duration = if self.reduced { 0.08 } else { 0.14 };
        let fade_start = self.painted.unwrap_or(start + PAINT_LIMIT);
        let blend = smooth((now - fade_start) / fade_duration);
        // Retain a little depth until fresh pixels arrive. Even a slow paint
        // then crossfades DURING perspective recovery, rather than afterwards.
        let flatten = 0.7 * ease_out(elapsed / 0.12) + 0.3 * blend;
        // Keep both layouts on the SAME plane and at the SAME scale throughout
        // the crossfade. Only the final, newly painted layout expands to 1:1.
        let settle_start =
            (fade_start + fade_duration).max(start + if self.reduced { 0. } else { 0.18 });
        let settle = if self.reduced {
            1.
        } else {
            ease_out((now - settle_start) / 0.16)
        };
        let pose = Pose {
            scale: from.scale + (1. - from.scale) * settle,
            x: from.x * (1. - flatten),
            y: from.y * (1. - flatten),
        };
        self.scale.value = pose.scale;
        self.x.value = pose.x;
        self.y.value = pose.y;
        Sample {
            pose,
            blend,
            finished: blend >= 1. && settle >= 1.,
        }
    }
}

/// Homogeneous projection about the page centre. Positive X depth makes the
/// left edge recede; positive Y depth makes the top edge recede. The edge next
/// to the pointer therefore gets the larger perspective denominator.
pub fn projection(pose: Pose, width: f32, height: f32) -> [[f32; 4]; 4] {
    let cx = width / 2.;
    let cy = height / 2.;
    let a = -pose.x / width.max(1.);
    let b = -pose.y / height.max(1.);
    let c = 1. + (pose.x.abs() + pose.y.abs()) / 2. + (pose.x + pose.y) / 2.;
    let s = pose.scale;
    [
        [s + cx * a, cy * a, 0., a],
        [cx * b, s + cy * b, 0., b],
        [0., 0., 1., 0.],
        [cx * (c - s), cy * (c - s), 0., c],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dragged() -> Motion {
        let mut motion = Motion::new(false);
        motion.point(-1., -0.4);
        for n in 0..30 {
            motion.sample(n as f32 / 60., 1. / 60.);
        }
        motion
    }
    #[test]
    fn pointer_side_recedes_and_drag_spring_settles_without_overshoot() {
        let mut m = dragged();
        let p = m.sample(0.5, 0.016).pose;
        assert!((0.968..0.969).contains(&p.scale));
        assert!(p.x > 0.12 && p.y > 0.);
        let matrix = projection(p, 1000., 700.);
        assert!(matrix[3][3] > 1000. * matrix[0][3] + matrix[3][3]);
        assert!(matrix[3][3] > 700. * matrix[1][3] + matrix[3][3]);
        assert_eq!(
            projection(Pose::default(), 1000., 700.),
            [
                [1., 0., 0., 0.],
                [0., 1., 0., 0.],
                [0., 0., 1., 0.],
                [0., 0., 0., 1.]
            ]
        );
    }
    #[test]
    fn flatten_and_crossfade_overlap_before_final_scale() {
        let mut m = dragged();
        m.release(1.);
        m.paint_ready(1.04);
        let early = m.sample(1.10, 0.016);
        assert!(early.pose.x > 0. && early.pose.x < 0.03);
        assert!(early.blend > 0. && early.blend < 1.);
        assert!(early.pose.scale < 0.97);
        let settle = m.sample(1.24, 0.016);
        assert_eq!(settle.blend, 1.);
        assert_eq!(settle.pose.x, 0.);
        assert!(settle.pose.scale > 0.97 && settle.pose.scale < 1.);
        assert!(m.sample(1.36, 0.016).finished);
    }
    #[test]
    fn missing_capture_and_reduced_motion_always_finish() {
        let mut m = dragged();
        m.release(1.);
        let waiting = m.sample(1.20, 0.016);
        assert!(!waiting.finished);
        assert!(
            waiting.pose.x > 0.,
            "keep perspective until the paint handoff"
        );
        assert!(m.sample(1.55, 0.016).finished);
        let mut m = Motion::new(true);
        m.point(-1., 1.);
        assert_eq!(m.sample(0.1, 0.1).pose.scale, 1.);
        m.release(1.);
        m.paint_ready(1.04);
        assert!(m.sample(1.13, 0.016).finished);
    }
    #[test]
    fn split_entry_mirrors_depth_and_waits_for_paint_before_clearing_blur() {
        let mut left = Motion::new(false);
        let mut right = Motion::new(false);
        left.enter(1.);
        right.enter(-1.);
        assert!(!left.dragging());
        let a = left.sample(0., 0.);
        let b = right.sample(0., 0.);
        assert_eq!(a.pose.scale, 0.968);
        assert_eq!(a.pose.x, -b.pose.x);
        assert!(a.pose.x < 0.);
        assert_eq!(a.blend, 0.);
        // Late pixels retain the lifted scale (and therefore content blur).
        assert_eq!(left.sample(0.20, 0.016).pose.scale, 0.968);
        left.paint_ready(0.21);
        let fading = left.sample(0.28, 0.016);
        assert!(fading.blend > 0. && fading.blend < 1.);
        assert_eq!(fading.pose.scale, 0.968);
        let settling = left.sample(0.40, 0.016);
        assert!(settling.pose.scale > 0.968 && settling.pose.scale < 1.);
        let done = left.sample(0.52, 0.016);
        assert!(done.finished);
        assert_eq!(done.pose.scale, 1.);
        assert_eq!(done.pose.x, 0.);
        assert!(
            right.sample(0.56, 0.016).finished,
            "missing pixels must finish"
        );
    }
    #[test]
    fn reduced_split_entry_only_crossfades() {
        let mut m = Motion::new(true);
        m.enter(1.);
        let first = m.sample(0., 0.);
        assert_eq!(first.pose.scale, 1.);
        assert_eq!(first.pose.x, 0.);
        m.paint_ready(0.04);
        assert!(m.sample(0.13, 0.016).finished);
    }
}

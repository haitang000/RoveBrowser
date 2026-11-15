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
    staged: bool,
    incoming: bool,
    blur_only: bool,
    blur: Spring,
    released_blur: f32,
    pub reduced: bool,
}
pub struct Sample {
    pub pose: Pose,
    pub blend: f32,
    pub finished: bool,
    pub opacity: f32,
    pub slide: f32,
    /// Content blur, independent of the card's projection and outline.
    pub blur: f32,
}
pub const MOVE_DURATION: f32 = 0.280;
/// Zero velocity and acceleration at both ends, with exact integer endpoints.
pub fn move_bounds(from: [i32; 4], to: [i32; 4], progress: f32) -> [i32; 4] {
    let t = progress.clamp(0., 1.) as f64;
    let t = t * t * t * (t * (t * 6. - 15.) + 10.);
    std::array::from_fn(|i| (from[i] as f64 + (to[i] as f64 - from[i] as f64) * t).round() as i32)
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
            staged: false,
            incoming: false,
            blur_only: false,
            blur: Spring::default(),
            released_blur: 0.,
            reduced,
        }
    }
    pub fn blur_only(reduced: bool, amount: f32) -> Self {
        Self {
            blur_only: true,
            blur: Spring {
                value: if reduced { 0. } else { amount.clamp(0., 1.) },
                velocity: 0.,
            },
            ..Self::new(reduced)
        }
    }
    pub fn dragging(&self) -> bool {
        self.released.is_none() && !self.staged
    }
    pub fn resume(&mut self, pose: Pose) {
        self.scale.value = pose.scale;
        self.x.value = pose.x;
        self.y.value = pose.y;
    }
    /// A newly shown split pane starts lifted beside the divider, then uses
    /// the same paint handoff and nonlinear settle as a released resize.
    pub fn enter(&mut self, edge: f32) {
        self.stage(edge);
        self.reveal(0.);
    }
    pub fn stage(&mut self, edge: f32) {
        self.point(edge, 0.);
        self.resume(self.target);
        self.staged = true;
        self.incoming = true;
    }
    pub fn reveal(&mut self, now: f32) {
        self.staged = false;
        self.release(now);
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
        self.released_blur = self.blur.value;
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
        if self.staged {
            return Sample {
                pose: self.pose(),
                blend: 0.,
                finished: false,
                opacity: 0.,
                slide: 0.08,
                blur: if self.reduced { 0. } else { 1. },
            };
        }
        let Some((start, from)) = self.released else {
            self.scale.step(self.target.scale, dt);
            self.x.step(self.target.x, dt);
            self.y.step(self.target.y, dt);
            if self.blur_only {
                self.blur.step(if self.reduced { 0. } else { 1. }, dt);
            }
            return Sample {
                pose: self.pose(),
                blend: 0.,
                finished: false,
                opacity: 1.,
                slide: 0.,
                blur: if self.blur_only {
                    self.blur.value
                } else {
                    ((1. - self.scale.value) / 0.032).clamp(0., 1.)
                },
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
        let blur = if self.blur_only {
            self.released_blur * (1. - settle)
        } else {
            ((1. - pose.scale) / 0.032).clamp(0., 1.)
        };
        self.blur.value = blur;
        Sample {
            pose,
            blend,
            finished: blend >= 1. && settle >= 1.,
            blur,
            opacity: if self.incoming {
                smooth(elapsed / if self.reduced { 0.08 } else { 0.14 })
            } else {
                1.
            },
            slide: if self.incoming && !self.reduced {
                0.08 * (1. - ease_out(elapsed / 0.24))
            } else {
                0.
            },
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
    #[test]
    fn sidebar_blurs_without_changing_the_plane_and_clears_after_paint() {
        let mut motion = Motion::blur_only(false, 0.);
        let identity = projection(Pose::default(), 1000., 700.);
        let mut blur = 0.;
        for n in 1..=12 {
            let sample = motion.sample(n as f32 / 60., 1. / 60.);
            assert_eq!(projection(sample.pose, 1000., 700.), identity);
            assert!(sample.blur >= blur && sample.blur <= 1.);
            blur = sample.blur;
        }
        assert!(blur > 0.9);
        motion.release(0.2);
        motion.paint_ready(0.24);
        let fading = motion.sample(0.31, 0.016);
        assert!(fading.blend > 0. && fading.blend < 1.);
        assert_eq!(fading.blur, blur, "retain blur through the layout crossfade");
        let clearing = motion.sample(0.44, 0.016);
        assert_eq!(projection(clearing.pose, 1000., 700.), identity);
        assert!(clearing.blur > 0. && clearing.blur < blur);
        let resumed = Motion::blur_only(false, clearing.blur).sample(0., 0.);
        assert!((resumed.blur - clearing.blur).abs() < 1e-6, "a new toggle keeps the visible blur");
        let done = motion.sample(0.6, 0.016);
        assert!(done.finished);
        assert_eq!(done.blur, 0.);
        for reduced in [false, true] {
            let mut missing_capture = Motion::blur_only(reduced, blur);
            missing_capture.sample(0.2, 0.2);
            missing_capture.release(0.2);
            let last = missing_capture.sample(0.8, 0.016);
            assert!(last.finished);
            assert_eq!(last.blur, 0.);
            assert_eq!(projection(last.pose, 1000., 700.), identity);
        }
    }
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
    #[test]
    fn incoming_page_stays_hidden_until_outgoing_page_has_arrived() {
        let mut incoming = Motion::new(false);
        incoming.stage(-1.);
        assert!(!incoming.dragging());
        for now in [0., 0.2, 0.5] {
            let sample = incoming.sample(now, 0.016);
            assert_eq!(sample.opacity, 0.);
            assert!(!sample.finished);
        }
        incoming.reveal(0.5);
        let first = incoming.sample(0.5, 0.);
        assert_eq!(first.opacity, 0.);
        assert_eq!(first.slide, 0.08);
        let middle = incoming.sample(0.57, 0.016);
        assert!(middle.opacity > 0. && middle.opacity < 1.);
        assert!(middle.slide > 0. && middle.slide < first.slide);
        incoming.paint_ready(0.55);
        let last = incoming.sample(0.9, 0.016);
        assert_eq!(last.opacity, 1.);
        assert_eq!(last.slide, 0.);
        assert!(last.finished);
    }
    #[test]
    fn original_rectangle_moves_continuously_between_exact_endpoints() {
        let from = [240, 40, 1000, 760];
        let to = [240, 70, 496, 730];
        assert_eq!(move_bounds(from, to, 0.), from);
        assert_eq!(move_bounds(from, to, 1.), to);
        let mut previous = from;
        for n in 1..=30 {
            let bounds = move_bounds(from, to, n as f32 / 30.);
            assert!(bounds[2] <= previous[2] && bounds[2] >= to[2]);
            assert!(bounds[1] >= previous[1] && bounds[1] <= to[1]);
            previous = bounds;
        }
        assert_eq!(move_bounds(from, to, 0.01), from);
        assert_eq!(move_bounds(from, to, 0.99), to);
    }
}

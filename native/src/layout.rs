//! Resize live pages before the shell renderer has produced its next layout.
use crate::model::Rect;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct Viewport {
    pub width: f64,
    pub height: f64,
}
impl Viewport {
    pub fn valid(self) -> bool {
        [self.width, self.height]
            .into_iter()
            .all(|n| n.is_finite() && n > 0. && n < 32000.)
    }

    pub fn matches(self, current: Self) -> bool {
        // Chromium reports integer CSS pixels; winit retains fractional DPI units.
        self.valid()
            && (self.width - current.width).abs() < 1.
            && (self.height - current.height).abs() < 1.
    }
}

fn sidebar(viewport: Viewport, preferred: Option<f64>) -> f64 {
    let width = preferred.unwrap_or(if viewport.width <= 740. { 216. } else { 240. });
    width.min((viewport.width - 248.).clamp(192., 480.))
}

/// Preserve the DOM's borders, caption/header heights, split share and gap.
/// Only the flex dimensions change. The next current-viewport DOM report remains
/// authoritative for transitions and changes to the shell's appearance.
pub fn resized(
    primary: Rect,
    secondary: Option<Rect>,
    previous: Viewport,
    current: Viewport,
    preferred_sidebar: Option<f64>,
    collapsed: bool,
) -> (Rect, Option<Rect>) {
    let dx = if collapsed {
        0.
    } else {
        sidebar(current, preferred_sidebar) - sidebar(previous, preferred_sidebar)
    };
    let dw = current.width - previous.width - dx;
    let dh = current.height - previous.height;
    let mut left = Rect {
        x: primary.x + dx,
        height: (primary.height + dh).max(0.),
        ..primary
    };
    let right = if let Some(right) = secondary {
        let gap = right.x - primary.x - primary.width;
        let total = primary.width + right.width;
        let width = (total + dw).max(0.);
        left.width = width * (primary.width / total.max(1.));
        Some(Rect {
            x: left.x + left.width + gap,
            width: width - left.width,
            height: (right.height + dh).max(0.),
            ..right
        })
    } else {
        left.width = (primary.width + dw).max(0.);
        None
    };
    (left, right)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_page_follows_window_and_responsive_sidebar() {
        let old = Viewport {
            width: 1280.,
            height: 820.,
        };
        let page = Rect {
            x: 241.,
            y: 39.,
            width: 1030.,
            height: 772.,
            corners: [7.; 4],
        };
        let small = Viewport {
            width: 700.,
            height: 600.,
        };
        let (next, second) = resized(page, None, old, small, None, false);
        assert!(second.is_none());
        assert_eq!(
            (next.x, next.y, next.width, next.height),
            (217., 39., 474., 552.)
        );
        assert_eq!(next.corners, page.corners);
        let (restored, _) = resized(next, None, small, old, None, false);
        assert_eq!(
            (restored.x, restored.width, restored.height),
            (page.x, page.width, page.height)
        );
    }

    #[test]
    fn split_keeps_shared_seam_and_custom_sidebar_clamps_then_recovers() {
        let old = Viewport {
            width: 1280.,
            height: 820.,
        };
        let small = Viewport {
            width: 480.,
            height: 400.,
        };
        let left = Rect {
            x: 481.,
            y: 69.,
            width: 300.,
            height: 742.,
            ..Rect::default()
        };
        let right = Rect {
            x: 789.,
            width: 482.,
            ..left
        };
        let (a, b) = resized(left, Some(right), old, small, Some(480.), false);
        let b = b.unwrap();
        assert_eq!(a.x, 233.);
        assert!((b.x - a.x - a.width - 8.).abs() < 0.001);
        assert!((b.x + b.width - 471.).abs() < 0.001);
        assert!((a.width / b.width - left.width / right.width).abs() < 0.001);
        assert_eq!((a.y, a.height, b.height), (69., 322., 322.));
        let (a, b) = resized(a, Some(b), small, old, Some(480.), false);
        assert_eq!(a.x, left.x);
        assert!((b.unwrap().width - right.width).abs() < 0.001);
    }

    #[test]
    fn collapsed_sidebar_does_not_shift_page_at_responsive_breakpoint() {
        let page = Rect {
            x: 9.,
            width: 1262.,
            height: 772.,
            ..Rect::default()
        };
        let (next, _) = resized(
            page,
            None,
            Viewport {
                width: 1280.,
                height: 820.,
            },
            Viewport {
                width: 480.,
                height: 400.,
            },
            None,
            true,
        );
        assert_eq!((next.x, next.width, next.height), (9., 462., 352.));
    }

    #[test]
    fn reject_old_or_invalid_viewports_but_allow_dpi_rounding() {
        let current = Viewport {
            width: 1000.8,
            height: 700.4,
        };
        assert!(Viewport {
            width: 1001.,
            height: 700.
        }
        .matches(current));
        for old in [
            Viewport {
                width: 1280.,
                height: 820.,
            },
            Viewport {
                width: f64::NAN,
                height: 700.,
            },
            Viewport {
                width: 0.,
                height: 700.,
            },
        ] {
            assert!(!old.matches(current));
        }
    }
}

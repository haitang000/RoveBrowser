//! Zigzag `Tabs/TabSleep.swift`, adapted to WebView2: when a background page
//! gives its memory back.
//!
//! A page nobody has looked at for a while is suspended with
//! `ICoreWebView2_3::TrySuspend`, so its renderer can release what it holds; the
//! page stays where it was and wakes when it is shown again. The rules are
//! Zigzag's: a tab out of sight sleeps after an hour, after a quarter of an hour
//! once more than 20 are awake, and after five minutes only while the machine is
//! short of memory. A page that is showing, playing sound, loading or part of
//! the split is never suspended; a favorite waits for critical memory only.
//!
//! Thresholds and the tick length can be shortened through `ERA_SLEEP_*`
//! variables in debug builds, so the native smoke test can watch a real
//! suspension instead of waiting an hour. Release builds always use Zigzag's
//! values.
use std::time::Duration;

/// Zigzag `TabSleep.idle`.
pub const IDLE: Duration = Duration::from_secs(60 * 60);
/// Zigzag `TabSleep.crowd`: more awake pages than this and idle ones sleep sooner.
pub const CROWD: usize = 20;
/// Zigzag `TabSleep.crowdedIdle`.
pub const CROWDED_IDLE: Duration = Duration::from_secs(15 * 60);
/// Zigzag `TabSleep.lowOnMemoryIdle`.
pub const LOW_ON_MEMORY_IDLE: Duration = Duration::from_secs(5 * 60);
/// A failed suspension is not retried on every tick.
pub const RETRY_AFTER: Duration = Duration::from_secs(10 * 60);
/// How often the host looks for pages to sleep.
const TICK: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Memory {
    Normal,
    /// The machine is short of memory: idle pages sleep after five minutes.
    Low,
    /// Critically short: even a favorite may sleep, one at a time.
    Critical,
}

impl Memory {
    fn is_low(self) -> bool {
        self != Self::Normal
    }
}

/// What the decision needs to know about one loaded page.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub id: String,
    /// How long since the tab it shows was last looked at.
    pub unseen: Duration,
    /// False while the page is on screen, playing sound, loading, or waiting on a
    /// failed suspension.
    pub can_sleep: bool,
    /// A favorite's page stays as the user left it until memory is critical.
    pub favorite: bool,
}

#[derive(Clone, Debug)]
pub struct Thresholds {
    pub idle: Duration,
    pub crowd: usize,
    pub crowded_idle: Duration,
    pub low_on_memory_idle: Duration,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            idle: IDLE,
            crowd: CROWD,
            crowded_idle: CROWDED_IDLE,
            low_on_memory_idle: LOW_ON_MEMORY_IDLE,
        }
    }
}

/// Zigzag `TabSleep.sleepers`: the pages to suspend now, longest unseen first.
/// `awake` is how many of the tab's pages still hold a live WebView.
pub fn sleepers(
    candidates: &[Candidate],
    awake: usize,
    memory: Memory,
    thresholds: &Thresholds,
) -> Vec<String> {
    let mut awake = awake;
    let mut sleeping = Vec::new();
    let low = memory.is_low();
    let mut ordered: Vec<&Candidate> = candidates.iter().collect();
    ordered.sort_by_key(|candidate| std::cmp::Reverse(candidate.unseen));
    for candidate in ordered {
        if !candidate.can_sleep {
            continue;
        }
        if candidate.favorite {
            // Zigzag: "A favorite never sleeps for being idle or one of many ...
            // Only memory running critically short puts it to sleep."
            if !(memory == Memory::Critical && candidate.unseen > thresholds.low_on_memory_idle) {
                continue;
            }
        } else if !(candidate.unseen > thresholds.idle
            || (low && candidate.unseen > thresholds.low_on_memory_idle)
            || (awake > thresholds.crowd && candidate.unseen > thresholds.crowded_idle))
        {
            continue;
        }
        sleeping.push(candidate.id.clone());
        awake = awake.saturating_sub(1);
    }
    sleeping
}

/// Windows memory load to Zigzag's pressure levels. Zigzag asks macOS; the
/// desktop equivalent is `MEMORYSTATUSEX.dwMemoryLoad`, and the cut-offs are the
/// levels at which Windows itself starts trimming working sets.
pub fn level(memory_load_percent: u32) -> Memory {
    if memory_load_percent >= 96 {
        Memory::Critical
    } else if memory_load_percent >= 90 {
        Memory::Low
    } else {
        Memory::Normal
    }
}

#[cfg(target_os = "windows")]
pub fn pressure() -> Memory {
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    // SAFETY: `status` declares its own size and is owned by this call.
    if unsafe { GlobalMemoryStatusEx(&mut status) }.is_ok() {
        level(status.dwMemoryLoad)
    } else {
        Memory::Normal
    }
}

#[cfg(not(target_os = "windows"))]
pub fn pressure() -> Memory {
    Memory::Normal
}

/// Zigzag's values; debug builds may shorten them for the native smoke test.
pub fn thresholds() -> Thresholds {
    #[cfg(debug_assertions)]
    {
        let mut thresholds = Thresholds::default();
        if let Some(seconds) = env_secs("ERA_SLEEP_IDLE_SECS") {
            thresholds.idle = seconds;
        }
        if let Some(seconds) = env_secs("ERA_SLEEP_CROWDED_SECS") {
            thresholds.crowded_idle = seconds;
        }
        if let Some(seconds) = env_secs("ERA_SLEEP_LOW_SECS") {
            thresholds.low_on_memory_idle = seconds;
        }
        if let Some(crowd) = std::env::var("ERA_SLEEP_CROWD")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|crowd| *crowd >= 1)
        {
            thresholds.crowd = crowd;
        }
        thresholds
    }
    #[cfg(not(debug_assertions))]
    {
        Thresholds::default()
    }
}

/// `ERA_SLEEP_TICK_SECS` shortens the wait for the smoke test, between one
/// second and five minutes.
pub fn tick_interval() -> Duration {
    #[cfg(debug_assertions)]
    if let Some(seconds) = env_secs("ERA_SLEEP_TICK_SECS") {
        return seconds.min(Duration::from_secs(300));
    }
    TICK
}

#[cfg(debug_assertions)]
fn env_secs(name: &str) -> Option<Duration> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| *seconds >= 1)
        .map(Duration::from_secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str, unseen_secs: u64) -> Candidate {
        Candidate {
            id: id.into(),
            unseen: Duration::from_secs(unseen_secs),
            can_sleep: true,
            favorite: false,
        }
    }

    #[test]
    fn idle_pages_sleep_after_an_hour() {
        let thresholds = Thresholds::default();
        let quiet = [candidate("quiet", 30 * 60)];
        assert!(sleepers(&quiet, 5, Memory::Normal, &thresholds).is_empty());
        let stale = [candidate("stale", 61 * 60)];
        assert_eq!(sleepers(&stale, 5, Memory::Normal, &thresholds), ["stale"]);
    }

    #[test]
    fn a_crowd_lowers_the_wait() {
        let thresholds = Thresholds::default();
        let candidates = [candidate("page", 16 * 60)];
        assert!(
            sleepers(&candidates, thresholds.crowd, Memory::Normal, &thresholds).is_empty(),
            "20 awake pages keep an ordinary tab for an hour"
        );
        assert_eq!(
            sleepers(&candidates, thresholds.crowd + 1, Memory::Normal, &thresholds),
            ["page"],
            "21 awake pages let a quarter-hour-idle tab go"
        );
    }

    #[test]
    fn low_memory_sleeps_sooner_but_favorites_wait_for_critical() {
        let thresholds = Thresholds::default();
        let candidates = [
            candidate("page", 6 * 60),
            Candidate {
                favorite: true,
                ..candidate("favorite", 6 * 60)
            },
        ];
        let ordinary = sleepers(&candidates, 5, Memory::Low, &thresholds);
        assert_eq!(ordinary, ["page"]);
        let critical = sleepers(&candidates, 5, Memory::Critical, &thresholds);
        assert_eq!(critical, ["page", "favorite"]);
    }

    #[test]
    fn pages_that_must_stay_never_sleep() {
        let thresholds = Thresholds::default();
        let candidates = [
            Candidate {
                can_sleep: false,
                ..candidate("on-screen", 24 * 60 * 60)
            },
            Candidate {
                can_sleep: false,
                ..candidate("playing", 24 * 60 * 60)
            },
        ];
        assert!(sleepers(&candidates, 30, Memory::Critical, &thresholds).is_empty());
    }

    #[test]
    fn the_longest_unseen_go_first() {
        let thresholds = Thresholds::default();
        let candidates = [
            candidate("recent", 61 * 60),
            candidate("oldest", 4 * 60 * 60),
            candidate("older", 2 * 60 * 60),
        ];
        assert_eq!(
            sleepers(&candidates, 3, Memory::Normal, &thresholds),
            // Once each leaves, the awake count falls, but a normal-memory pass
            // has no crowd condition to lose, so all three go oldest first.
            ["oldest", "older", "recent"]
        );
    }

    #[test]
    fn a_crowd_only_sleeps_as_many_as_needed() {
        let thresholds = Thresholds {
            crowd: 20,
            ..Thresholds::default()
        };
        let candidates: Vec<_> = (0..25).map(|i| candidate(&format!("tab-{i}"), 16 * 60)).collect();
        let sleeping = sleepers(&candidates, 25, Memory::Normal, &thresholds);
        assert_eq!(sleeping.len(), 5, "the awake count falls to the crowd size");
    }

    #[test]
    fn memory_load_maps_to_zigzag_levels() {
        assert_eq!(level(0), Memory::Normal);
        assert_eq!(level(70), Memory::Normal);
        assert_eq!(level(90), Memory::Low);
        assert_eq!(level(95), Memory::Low);
        assert_eq!(level(96), Memory::Critical);
        assert_eq!(level(100), Memory::Critical);
    }

    #[cfg(debug_assertions)]
    #[test]
    fn debug_env_can_shorten_the_thresholds() {
        std::env::set_var("ERA_SLEEP_IDLE_SECS", "2");
        std::env::set_var("ERA_SLEEP_CROWD", "3");
        std::env::set_var("ERA_SLEEP_TICK_SECS", "1");
        let thresholds = thresholds();
        assert_eq!(thresholds.idle, Duration::from_secs(2));
        assert_eq!(thresholds.crowd, 3);
        assert_eq!(tick_interval(), Duration::from_secs(1));
        std::env::remove_var("ERA_SLEEP_IDLE_SECS");
        std::env::remove_var("ERA_SLEEP_CROWD");
        std::env::remove_var("ERA_SLEEP_TICK_SECS");
    }
}

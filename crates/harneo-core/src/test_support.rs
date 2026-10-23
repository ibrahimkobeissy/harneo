//! Helpers shared by the unit tests.

use std::time::{Duration, Instant};

/// A performance budget from a spec, as a test asserts it. The coverage build
/// (`cargo llvm-cov`, which sets `cfg(coverage)`) instruments every line and runs
/// several times slower, so there the same budget is allowed four times as long;
/// normal test runs assert the spec's number unchanged.
pub(crate) fn budget(spec: Duration) -> Duration {
    if cfg!(coverage) {
        spec * 4
    } else {
        spec
    }
}

/// Times the code a test runs on its own thread. On Linux it reads the thread's CPU
/// time, so a machine saturated by other builds doesn't count against a budget; elsewhere
/// (or if the kernel file can't be read) it falls back to the wall clock.
pub(crate) struct Stopwatch {
    wall: Instant,
    cpu: Option<Duration>,
}

impl Stopwatch {
    pub(crate) fn start() -> Self {
        Self {
            wall: Instant::now(),
            cpu: thread_cpu(),
        }
    }

    pub(crate) fn elapsed(&self) -> Duration {
        match (self.cpu, thread_cpu()) {
            (Some(start), Some(now)) => now.saturating_sub(start),
            _ => self.wall.elapsed(),
        }
    }
}

/// The calling thread's CPU time: the first field of `/proc/thread-self/schedstat`, in ns.
fn thread_cpu() -> Option<Duration> {
    let text = std::fs::read_to_string("/proc/thread-self/schedstat").ok()?;
    let ns: u64 = text.split_whitespace().next()?.parse().ok()?;
    Some(Duration::from_nanos(ns))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_budget_is_the_spec_outside_coverage_and_four_times_it_inside() {
        let spec = Duration::from_millis(100);
        let expected = if cfg!(coverage) { spec * 4 } else { spec };
        assert_eq!(budget(spec), expected);
    }

    #[test]
    fn the_stopwatch_counts_work_done_on_this_thread() {
        let wall = Instant::now();
        let watch = Stopwatch::start();
        let mut x: u64 = 1;
        for _ in 0..20_000_000u64 {
            x = std::hint::black_box(x.wrapping_mul(6364136223846793005).wrapping_add(1));
        }
        let cpu = watch.elapsed();
        assert!(x != 0);
        assert!(cpu > Duration::ZERO);
        assert!(cpu <= wall.elapsed() + Duration::from_millis(5), "{cpu:?}");
    }

    #[test]
    fn the_stopwatch_does_not_count_sleeping() {
        let watch = Stopwatch::start();
        std::thread::sleep(Duration::from_millis(80));
        if thread_cpu().is_some() {
            assert!(
                watch.elapsed() < Duration::from_millis(40),
                "{:?}",
                watch.elapsed()
            );
        }
    }
}

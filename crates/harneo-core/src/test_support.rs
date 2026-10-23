//! Helpers shared by the unit tests.

use std::time::Duration;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_budget_is_the_spec_outside_coverage_and_four_times_it_inside() {
        let spec = Duration::from_millis(100);
        let expected = if cfg!(coverage) { spec * 4 } else { spec };
        assert_eq!(budget(spec), expected);
    }
}

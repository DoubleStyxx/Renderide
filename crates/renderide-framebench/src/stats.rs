//! Linear-interpolation percentiles.
//!
//! Rank is `(pct / 100) * (n - 1)`. The p50 of an even count is the average of the two
//! central samples, which is the median used when collapsing N runs.

/// p50, p95, and p99 of one sample set.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Percentiles {
    /// Median. Even counts average the two central samples.
    pub p50: f64,
    /// 95th percentile.
    pub p95: f64,
    /// 99th percentile.
    pub p99: f64,
}

/// Percentiles of `values`. Empty input returns [`None`].
pub fn percentiles(values: &[f64]) -> Option<Percentiles> {
    Some(Percentiles {
        p50: percentile(values, 50.0)?,
        p95: percentile(values, 95.0)?,
        p99: percentile(values, 99.0)?,
    })
}

/// Linear-interpolation percentile in `0..=100`.
pub fn percentile(values: &[f64], pct: f64) -> Option<f64> {
    if values.is_empty() || !pct.is_finite() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let last = sorted.len().checked_sub(1)?;
    if last == 0 {
        return sorted.first().copied();
    }
    let span = u32::try_from(last).ok()?;
    let rank = (pct / 100.0) * f64::from(span);
    let lo = rank.floor();
    let hi = rank.ceil();
    let lo_idx = float_index(lo, sorted.len())?;
    let hi_idx = float_index(hi, sorted.len())?;
    let low = sorted.get(lo_idx).copied()?;
    let high = sorted.get(hi_idx).copied()?;
    let weight = rank - lo;
    let mixed = low + (high - low) * weight;
    mixed.is_finite().then_some(mixed)
}

/// Median. Even counts average the two central samples.
pub fn median(values: &[f64]) -> Option<f64> {
    percentile(values, 50.0)
}

fn float_index(value: f64, len: usize) -> Option<usize> {
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    let index = usize::try_from(value.round() as u64).ok()?;
    (index < len).then_some(index)
}

#[cfg(test)]
mod tests {
    use super::{median, percentile, percentiles};

    #[test]
    fn single_sample_is_every_percentile() {
        let stats = percentiles(&[4.0]).expect("one");
        assert_eq!(stats.p50, 4.0);
        assert_eq!(stats.p95, 4.0);
        assert_eq!(stats.p99, 4.0);
    }

    #[test]
    fn even_median_averages_the_two_central_samples() {
        assert_eq!(median(&[1.0, 2.0, 3.0, 4.0]), Some(2.5));
    }

    #[test]
    fn p95_interpolates() {
        let value = percentile(&[0.0, 10.0, 20.0], 95.0).expect("p95");
        assert!((value - 19.0).abs() < 1e-9, "{value}");
    }

    #[test]
    fn empty_input_is_none() {
        assert!(percentiles(&[]).is_none());
    }
}

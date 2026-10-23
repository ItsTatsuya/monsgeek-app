//! Estimate the USB polling rate from key-change timestamps. Changes in one report arrive
//! together; separate reports can never be closer than one polling interval, and the gaps
//! between them cluster at multiples of it.

use crate::protocol::REPORT_RATES;

pub const BIN_US: f64 = 62.5;
pub const BINS: usize = 32;
/// Key changes closer than this arrived in the same USB report.
const SAME_REPORT_US: f64 = 40.0;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PollEstimate {
    /// Best-supported polling rate, or None while evidence is insufficient.
    pub hz: Option<u32>,
    /// Rate implied by the median gap around the chosen interval.
    pub measured_hz: Option<u32>,
    pub events: usize,
    pub reports: usize,
    /// Gaps between reports shorter than 20 ms, which carry the timing evidence.
    pub evidence: usize,
    /// Count of short gaps per 62.5 µs bin, from 0 to 2 ms.
    pub histogram: Vec<u32>,
}

pub fn estimate(times: &[f64]) -> PollEstimate {
    let mut sorted = times.to_vec();
    sorted.sort_by(f64::total_cmp);
    let mut reports: Vec<f64> = Vec::new();
    for &t in &sorted {
        if reports.last().is_none_or(|&last| t - last > SAME_REPORT_US) {
            reports.push(t);
        }
    }
    let gaps: Vec<f64> = reports
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|&g| g < 20_000.0)
        .collect();
    let mut histogram = vec![0u32; BINS];
    for &g in &gaps {
        let bin = (g / BIN_US) as usize;
        if bin < BINS {
            histogram[bin] += 1;
        }
    }
    let mut hz = None;
    let mut measured_hz = None;
    for &rate in &REPORT_RATES {
        let period = 1e6 / rate as f64;
        let too_close = gaps.iter().filter(|&&g| g < period * 0.8).count();
        let mut near: Vec<f64> = gaps
            .iter()
            .copied()
            .filter(|&g| g >= period * 0.8 && g < period * 1.6)
            .collect();
        if near.len() >= 3 && too_close as f64 <= (gaps.len() as f64 * 0.03).max(1.0) {
            near.sort_by(f64::total_cmp);
            hz = Some(rate);
            measured_hz = Some((1e6 / near[near.len() / 2]).round() as u32);
            break;
        }
    }
    PollEstimate {
        hz,
        measured_hz,
        events: sorted.len(),
        reports: reports.len(),
        evidence: gaps.len(),
        histogram,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bursts of simultaneous key changes spread over consecutive reports `period` µs apart.
    fn bursts(period: f64) -> Vec<f64> {
        let mut t = Vec::new();
        for burst in 0..10 {
            let start = burst as f64 * 100_000.0;
            for report in 0..4 {
                // Two keys in the same report, a few µs apart.
                t.push(start + report as f64 * period);
                t.push(start + report as f64 * period + 3.0);
            }
        }
        t
    }

    #[test]
    fn detects_1k_and_8k() {
        let e = estimate(&bursts(1000.0));
        assert_eq!(e.hz, Some(1000));
        assert_eq!(e.measured_hz, Some(1000));
        assert_eq!(e.reports, 40);
        assert_eq!(estimate(&bursts(125.0)).hz, Some(8000));
        assert_eq!(estimate(&[1.0, 5.0]).hz, None);
    }
}

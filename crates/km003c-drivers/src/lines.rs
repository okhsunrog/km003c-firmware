//! Passive board line voltages. Stock 2.0.7 uses one 1080 mV reference for
//! ADC1 channels 0..3 (CC1, CC2, D+, D-); there are no per-line gains here.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample {
    pub raw: [u16; 5],
    pub uv: [u32; 4],
    pub vdd_uv: u32,
}
impl Sample {
    /// Reject invalid reference/supply values rather than inventing 3.3 V.
    /// Reference accuracy is inherited from the stock scale, not a calibration
    /// against an external voltmeter. Full scale is 4096, not maximum code 4095.
    pub fn from_raw(raw: [u16; 5]) -> Option<Self> {
        if raw.iter().any(|&v| v > 4095) || raw[4] == 0 {
            return None;
        }
        let vdd_uv = 1_080_000u64 * 4096 / u64::from(raw[4]);
        if !(2_000_000..=3_600_000).contains(&vdd_uv) {
            return None;
        }
        let uv = core::array::from_fn(|i| {
            // Divide once: preserve precision when the measured supply varies.
            (u64::from(raw[i]) * 1_080_000 / u64::from(raw[4])) as u32
        });
        Some(Self {
            raw,
            uv,
            vdd_uv: vdd_uv as u32,
        })
    }
    /// Unique plausible active CC. Ambiguous, detached and VCONN-like levels
    /// return None. This is a passive receiver selector, not a Type-C policy.
    pub fn cc_input(&self) -> Option<u8> {
        let active = |v| (400_000..=2_150_000).contains(&v);
        match (active(self.uv[0]), active(self.uv[1])) {
            (true, false) => Some(1),
            (false, true) => Some(2),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stock_snapshot_and_common_scale() {
        let s = Sample::from_raw([0, 2052, 742, 0, 1343]).unwrap();
        assert_eq!(s.vdd_uv, 3_293_879);
        assert_eq!(s.uv, [0, 1_650_156, 596_693, 0]);
        assert_eq!(s.cc_input(), Some(2));
        // The four inputs must have identical transfer functions.
        let same = Sample::from_raw([1343; 5]).unwrap();
        assert_eq!(same.uv, [1_080_000; 4]);
    }
    #[test]
    fn invalid_reference_and_ambiguous_cc() {
        for raw in [[0; 5], [0, 0, 0, 0, 4095], [4096, 0, 0, 0, 1343]] {
            assert!(Sample::from_raw(raw).is_none());
        }
        assert_eq!(
            Sample::from_raw([2052, 0, 0, 0, 1343]).unwrap().cc_input(),
            Some(1)
        );
        assert_eq!(
            Sample::from_raw([2052, 2052, 0, 0, 1343])
                .unwrap()
                .cc_input(),
            None
        );
        assert_eq!(
            Sample::from_raw([0, 0, 0, 0, 1343]).unwrap().cc_input(),
            None
        );
        assert_eq!(
            Sample::from_raw([4095, 2052, 0, 0, 1343])
                .unwrap()
                .cc_input(),
            Some(2)
        );
    }
}

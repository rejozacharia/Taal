#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PeakLevel {
    pub max: f32,
    pub min: f32,
}

impl PeakLevel {
    pub fn silence() -> Self {
        Self { max: 0.0, min: 0.0 }
    }
}

pub fn normalize_buffer(buffer: &mut [f32]) -> PeakLevel {
    let mut peak = PeakLevel::silence();
    for sample in buffer.iter() {
        peak.max = peak.max.max(*sample);
        peak.min = peak.min.min(*sample);
    }
    let gain = peak.max.abs().max(peak.min.abs()).max(1e-6);
    for sample in buffer.iter_mut() {
        *sample /= gain;
    }
    peak
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_scales_to_unity() {
        let mut buffer = vec![0.5, -1.0, 0.75];
        let peak = normalize_buffer(&mut buffer);
        assert!((peak.max - 0.75).abs() < 1e-6);
        assert!(buffer.iter().all(|s| s.abs() <= 1.0));
    }
}

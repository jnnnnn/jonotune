//! Pitch detection via the YIN algorithm (simplified).
//!
//! For monophonic voice this is simple and reliable:
//! 1. Compute the squared difference function of a short window of samples.
//! 2. Apply cumulative mean normalization to penalize subharmonics.
//! 3. Find the lag with the smallest normalized difference → the period.
//! 4. `pitch_hz = sample_rate / lag`.
//!
//! Reference: De Cheveigné & Kawahara (2002), "YIN, a fundamental frequency
//! estimator for speech and music."

/// Detected pitch result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pitch {
    /// Frequency in Hz, or `None` if the signal is too quiet / unpitched.
    pub hz: Option<f32>,
    /// Confidence 0..1 (higher = more likely to be a real pitch).
    pub confidence: f32,
}

/// Detects the dominant pitch in a mono f32 sample buffer using the YIN algorithm.
pub struct PitchDetector {
    /// The sample rate the detector was configured for.
    sample_rate: u32,
    /// Minimum detectable frequency (Hz). Default: ~80 Hz (low male voice).
    min_freq: f32,
    /// Maximum detectable frequency (Hz). Default: ~1000 Hz (high soprano).
    max_freq: f32,
    /// RMS threshold below which we report silence.
    silence_threshold: f32,
}

impl PitchDetector {
    /// Create a new detector.
    ///
    /// * `sample_rate` - sample rate in Hz (e.g. 44100).
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            min_freq: 80.0,
            max_freq: 1000.0,
            silence_threshold: 0.01,
        }
    }

    /// Minimum number of samples needed for a reliable detection.
    ///
    /// We require at least 3 full periods of the lowest detectable frequency.
    pub fn min_samples(&self) -> usize {
        (4.0 * self.sample_rate as f32 / self.min_freq).ceil() as usize
    }

    /// Analyse a buffer of samples and return the detected pitch.
    ///
    /// `samples` should be at least `min_samples()` long.
    pub fn detect(&self, samples: &[f32]) -> Pitch {
        let n = samples.len();
        if n == 0 {
            return Pitch {
                hz: None,
                confidence: 0.0,
            };
        }

        // --- 1. Compute RMS energy ---
        let rms = {
            let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
            (sum_sq / n as f32).sqrt()
        };

        if rms < self.silence_threshold {
            return Pitch {
                hz: None,
                confidence: 0.0,
            };
        }

        // --- 2. YIN-style difference function ---
        // Compute raw AMDF, then apply cumulative mean normalization
        // to penalize subharmonic (longer-lag) matches.
        let min_lag = (self.sample_rate as f32 / self.max_freq).ceil() as usize;
        let max_lag = (self.sample_rate as f32 / self.min_freq).floor() as usize;

        if max_lag >= n {
            return Pitch {
                hz: None,
                confidence: 0.0,
            };
        }

        // Compute raw difference function.
        let mut diff: Vec<f32> = vec![0.0; max_lag + 1];
        for lag in 0..=max_lag {
            let mut sum = 0.0f32;
            let count = n - lag;
            for i in 0..count {
                let d = samples[i] - samples[i + lag];
                sum += d * d;
            }
            diff[lag] = sum;
        }

        // Cumulative mean normalized difference (YIN step 3).
        // cmnd[0] = 0 (by convention, we start at min_lag).
        let mut cmnd: Vec<f32> = vec![0.0; max_lag + 1];
        cmnd[0] = 1.0;
        let mut running_sum = 0.0f32;
        for lag in 1..=max_lag {
            running_sum += diff[lag];
            let avg = running_sum / lag as f32;
            cmnd[lag] = if avg > 0.0 { diff[lag] * lag as f32 / running_sum } else { 1.0 };
        }

        // Find the lag with the minimum CMND (in the valid range).
        // YIN step 4: find the first local minimum (valley) below threshold.
        // This avoids triggering on a downward slope before the true dip.
        let threshold = 0.2;
        let mut best_lag: Option<usize> = None;
        let mut best_val: f32 = f32::MAX;
        let mut global_best_lag: usize = min_lag;
        let mut global_best_val: f32 = f32::MAX;
        let mut prev_prev_val = f32::MAX;
        let mut prev_val = f32::MAX;

        for lag in min_lag..=max_lag {
            let val = cmnd[lag];

            // Track global minimum as fallback.
            if val < global_best_val {
                global_best_val = val;
                global_best_lag = lag;
            }

            // Detect a local minimum: prev_val is lower than both its neighbors
            // AND below threshold.
            if prev_val < threshold
                && prev_val <= prev_prev_val
                && prev_val <= val
                && best_lag.is_none()
            {
                best_lag = Some(lag - 1);
                best_val = prev_val;
                break;
            }

            prev_prev_val = prev_val;
            prev_val = val;
        }

        // Fallback: if no dip below threshold, use global minimum.
        if best_lag.is_none() {
            best_lag = Some(global_best_lag);
            best_val = global_best_val;
        }

        // Confidence: 1.0 - cmnd[best] clipped to 0..1.
        // A deep dip (cmnd ≈ 0) → high confidence.
        let confidence = (1.0 - best_val).clamp(0.0, 1.0);

        // --- 3. Parabolic interpolation (YIN step 4 continued) ---
        // Refine the lag estimate to sub-sample accuracy.
        let lag = best_lag.unwrap();
        let lag_fractional = if lag > min_lag && lag < max_lag {
            let prev = cmnd[lag - 1];
            let curr = cmnd[lag];
            let next = cmnd[lag + 1];
            let denom = 2.0 * (prev - 2.0 * curr + next);
            if denom.abs() > 1e-10 {
                let delta = (prev - next) / denom;
                lag as f32 + delta
            } else {
                lag as f32
            }
        } else {
            lag as f32
        };

        let hz = self.sample_rate as f32 / lag_fractional;

        Pitch {
            hz: Some(hz),
            confidence,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    /// Generate a sine wave at the given frequency.
    fn sine_wave(freq: f32, sample_rate: u32, num_samples: usize) -> Vec<f32> {
        (0..num_samples)
            .map(|i| (TAU * freq * i as f32 / sample_rate as f32).sin())
            .collect()
    }

    #[test]
    fn test_detect_sine_a4() {
        let sr = 44100;
        let detector = PitchDetector::new(sr);
        // A4 = 440 Hz; need enough samples for a full period at 80 Hz.
        let signal = sine_wave(440.0, sr, detector.min_samples());
        let pitch = detector.detect(&signal);
        assert!(pitch.hz.is_some(), "should detect A4");
        let hz = pitch.hz.unwrap();
        assert!((hz - 440.0).abs() < 15.0, "expected ~440 Hz, got {hz}");
        assert!(pitch.confidence > 0.5, "confidence too low: {}", pitch.confidence);
    }

    #[test]
    fn test_detect_sine_c4() {
        let sr = 44100;
        let detector = PitchDetector::new(sr);
        // C4 ≈ 261.63 Hz
        let signal = sine_wave(261.63, sr, detector.min_samples());
        let pitch = detector.detect(&signal);
        assert!(pitch.hz.is_some(), "should detect C4");
        let hz = pitch.hz.unwrap();
        assert!((hz - 261.63).abs() < 10.0, "expected ~262 Hz, got {hz}");
    }

    #[test]
    fn test_detect_low_e2() {
        let sr = 44100;
        let detector = PitchDetector::new(sr);
        // E2 ≈ 82.41 Hz — near the lower bound
        let signal = sine_wave(82.41, sr, detector.min_samples());
        let pitch = detector.detect(&signal);
        assert!(pitch.hz.is_some(), "should detect E2");
        let hz = pitch.hz.unwrap();
        assert!((hz - 82.41).abs() < 10.0, "expected ~82 Hz, got {hz}");
    }

    #[test]
    fn test_detect_silence() {
        let sr = 44100;
        let detector = PitchDetector::new(sr);
        let silence = vec![0.0f32; 1024];
        let pitch = detector.detect(&silence);
        assert!(pitch.hz.is_none(), "silence should yield no pitch");
        assert!(pitch.confidence < 0.1);
    }

    #[test]
    fn test_detect_noise() {
        let sr = 44100;
        let detector = PitchDetector::new(sr);
        // Very quiet noise — should be below threshold
        // Use a simple deterministic pseudo-random via a basic LCG.
        let mut seed: u32 = 42;
        let noise: Vec<f32> = (0..1024)
            .map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                ((seed as f32 / u32::MAX as f32) - 0.5) * 0.001
            })
            .collect();
        let pitch = detector.detect(&noise);
        assert!(pitch.hz.is_none(), "quiet noise should yield no pitch");
    }
}

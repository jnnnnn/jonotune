//! Pitch detection via autocorrelation.
//!
//! For monophonic voice, autocorrelation is simple and reliable:
//! 1. Compute the normalised autocorrelation of a short window of samples.
//! 2. Find the first peak after the zero-lag spike — its lag is the period.
//! 3. `pitch_hz = sample_rate / lag`.

/// Detected pitch result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pitch {
    /// Frequency in Hz, or `None` if the signal is too quiet / unpitched.
    pub hz: Option<f32>,
    /// Confidence 0..1 (higher = more likely to be a real pitch).
    pub confidence: f32,
}

/// Detects the dominant pitch in a mono f32 sample buffer using autocorrelation.
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

    /// Analyse a buffer of samples and return the detected pitch.
    ///
    /// `samples` should be at least long enough to contain one period of the
    /// lowest detectable frequency (e.g. `sample_rate / min_freq` ≈ 550 samples
    /// at 44100 Hz for 80 Hz).
    pub fn detect(&self, _samples: &[f32]) -> Pitch {
        // TODO: compute RMS energy; if below silence_threshold, return Pitch { hz: None, confidence: 0.0 }
        // TODO: compute normalised autocorrelation
        // TODO: find first peak after zero-lag within the min/max frequency range
        // TODO: convert lag → frequency; estimate confidence from peak height
        Pitch {
            hz: None,
            confidence: 0.0,
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
        // A4 = 440 Hz; need enough samples for a full period at 80 Hz
        let signal = sine_wave(440.0, sr, (sr as f32 / 80.0).ceil() as usize);
        let pitch = detector.detect(&signal);
        // TODO: uncomment once detector is implemented
        // assert!(pitch.hz.is_some());
        // assert!((pitch.hz.unwrap() - 440.0).abs() < 5.0);
        // assert!(pitch.confidence > 0.8);
        let _ = (pitch, signal); // silence unused warnings for now
    }

    #[test]
    fn test_detect_silence() {
        let sr = 44100;
        let detector = PitchDetector::new(sr);
        let silence = vec![0.0f32; 1024];
        let pitch = detector.detect(&silence);
        // TODO: uncomment
        // assert!(pitch.hz.is_none());
        // assert!(pitch.confidence < 0.1);
        let _ = (pitch, silence);
    }
}

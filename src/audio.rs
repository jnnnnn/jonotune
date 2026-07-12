//! Audio capture backends.
//!
//! Platform dispatch:
//! - **Native** (`cpal`): opens the default input device, streams f32 samples
//!   into a lock-free ring buffer that the UI thread can poll.
//! - **Wasm** (`web-sys`): hooks into the browser's `AudioContext` +
//!   `AnalyserNode`; the UI thread pulls time-domain data each frame.

// ---------------------------------------------------------------------------
// Shared trait
// ---------------------------------------------------------------------------

/// Platform-agnostic interface for capturing real-time audio.
pub trait AudioCapture {
    /// Sample rate in Hz (e.g. 44100).
    fn sample_rate(&self) -> u32;

    /// Fill `buf` with the most recent interleaved mono samples.
    /// Returns the number of samples actually written (may be less than `buf.len()`).
    fn read_samples(&mut self, buf: &mut [f32]) -> usize;
}

// ---------------------------------------------------------------------------
// Native backend
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
pub mod native {
    //! Native audio capture via `cpal` + `ringbuf`.
    //!
    //! Architecture:
    //! - `cpal` opens the default input device and runs a callback on a
    //!   high-priority audio thread.
    //! - The callback writes f32 samples into a lock-free `ringbuf`.
    //! - The UI thread drains the ring buffer each frame for pitch detection.

    use super::AudioCapture;

    /// Captures mono f32 samples from the default input device into a ring buffer.
    pub struct NativeAudio {
        sample_rate: u32,
        /// Shared ring buffer (producer side is in the audio callback).
        #[allow(dead_code)]
        buffer: std::sync::Arc<ringbuf::HeapRb<f32>>,
        /// Consumer handle for the UI thread to drain samples from.
        #[allow(dead_code)]
        consumer: ringbuf::HeapCons<f32>,
        /// Kept alive — dropping stops the stream.
        #[allow(dead_code)]
        _stream: cpal::Stream,
    }

    impl NativeAudio {
        /// Attempt to open the default input device and start streaming.
        ///
        /// # Errors
        /// Returns `None` if no input device is available.
        pub fn new() -> Option<Self> {
            // TODO: open default input device
            // TODO: configure to mono f32 at a sensible sample rate
            // TODO: spawn the audio callback that writes into the ring buffer
            // TODO: keep _stream alive so capture continues
            None
        }
    }

    impl AudioCapture for NativeAudio {
        fn sample_rate(&self) -> u32 {
            self.sample_rate
        }

        fn read_samples(&mut self, _buf: &mut [f32]) -> usize {
            // TODO: drain available samples from the ring buffer into `_buf`
            // Return number of samples actually read.
            0
        }
    }
}

// ---------------------------------------------------------------------------
// Wasm backend
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
pub mod wasm {
    use super::AudioCapture;

    /// Captures mono f32 samples from the browser's microphone via Web Audio API.
    pub struct WasmAudio {
        sample_rate: u32,
        // TODO: store AudioContext, AnalyserNode, buffer handles
    }

    impl WasmAudio {
        /// Request microphone access and create an `AudioContext` + `AnalyserNode` chain.
        ///
        /// This must be called from within an async context (user-gesture required).
        pub async fn new() -> Option<Self> {
            // TODO: request mic via `navigator.mediaDevices.getUserMedia`
            // TODO: create AudioContext + AnalyserNode
            // TODO: connect mic → AnalyserNode
            None
        }
    }

    impl AudioCapture for WasmAudio {
        fn sample_rate(&self) -> u32 {
            self.sample_rate
        }

        fn read_samples(&mut self, _buf: &mut [f32]) -> usize {
            // TODO: call `AnalyserNode.getByteTimeDomainData` and convert to f32
            0
        }
    }
}

// ---------------------------------------------------------------------------
// Platform re-export
// ---------------------------------------------------------------------------

/// Creates the appropriate `AudioCapture` for this platform.
///
/// **Native**: synchronous — attempts to open the default mic immediately.
/// **Wasm**: async — requires a user gesture to trigger `getUserMedia`.
#[cfg(not(target_arch = "wasm32"))]
pub fn create_audio_capture() -> Option<Box<dyn AudioCapture>> {
    native::NativeAudio::new().map(|a| Box::new(a) as Box<dyn AudioCapture>)
}

#[cfg(target_arch = "wasm32")]
pub async fn create_audio_capture() -> Option<Box<dyn AudioCapture>> {
    wasm::WasmAudio::new()
        .await
        .map(|a| Box::new(a) as Box<dyn AudioCapture>)
}

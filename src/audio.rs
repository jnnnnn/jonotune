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
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        use ringbuf::traits::{Consumer, Producer, Split};

        /// Ring buffer capacity in samples (≈ 370 ms at 44 100 Hz).
    const RING_CAPACITY: usize = 16384;

    /// Captures mono f32 samples from the default input device into a ring buffer.
    pub struct NativeAudio {
        sample_rate: u32,
        /// Consumer handle for the UI thread to drain samples from.
        consumer: ringbuf::HeapCons<f32>,
        /// Kept alive — dropping stops the stream.
        _stream: cpal::Stream,
    }

    impl NativeAudio {
        /// Attempt to open the default input device and start streaming.
        ///
        /// Returns `None` if no input device is available or configuration fails.
        pub fn new() -> Option<Self> {
            let host = cpal::default_host();
            let device = host.default_input_device()?;
            let supported_cfg = device.default_input_config().ok()?;
            let sample_rate = supported_cfg.sample_rate().0;

            log::info!(
                "Opening audio device: {} ({} Hz)",
                device.name().unwrap_or_else(|_| "unknown".into()),
                sample_rate
            );

            // Force mono f32.
            let config = cpal::StreamConfig {
                channels: 1,
                sample_rate: cpal::SampleRate(sample_rate),
                buffer_size: cpal::BufferSize::Default,
            };

            let ring = ringbuf::HeapRb::new(RING_CAPACITY);
            let (mut prod, cons) = ring.split();

            let stream = device
                .build_input_stream(
                    &config,
                    move |data: &[f32], _info: &cpal::InputCallbackInfo| {
                        for &sample in data {
                            // Silently drop if buffer is full — UI thread
                            // drains fast enough at 60 fps.
                            let _ = prod.try_push(sample);
                        }
                    },
                    move |err| {
                        log::error!("Audio input error: {err}");
                    },
                    None,
                )
                .ok()?;

            stream.play().ok()?;

            log::info!("Audio stream started at {sample_rate} Hz");

            Some(Self {
                sample_rate,
                consumer: cons,
                _stream: stream,
            })
        }
    }

    impl AudioCapture for NativeAudio {
        fn sample_rate(&self) -> u32 {
            self.sample_rate
        }

        fn read_samples(&mut self, buf: &mut [f32]) -> usize {
            let mut count = 0;
            for slot in buf.iter_mut() {
                match self.consumer.try_pop() {
                    Some(sample) => {
                        *slot = sample;
                        count += 1;
                    }
                    None => break,
                }
            }
            count
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

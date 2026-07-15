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
    use cpal::traits::{DeviceTrait as _, HostTrait as _, StreamTrait as _};
    use ringbuf::traits::{Consumer as _, Producer as _, Split as _};

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
                            _ = prod.try_push(sample);
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
    use web_sys::wasm_bindgen::JsCast;
    use web_sys::wasm_bindgen::JsValue;
    use web_sys::{
        AnalyserNode, AudioContext, MediaDevices, MediaStream, MediaStreamAudioSourceNode,
        MediaStreamConstraints,
    };

    /// Captures mono f32 samples from the browser's microphone via Web Audio API.
    ///
    /// Signal chain: `getUserMedia` → `AudioContext` → `MediaStreamAudioSourceNode`
    /// → `AnalyserNode`.  The analyser is *not* connected to `destination` so there
    /// is no feedback loop.
    pub struct WasmAudio {
        sample_rate: u32,
        analyser: AnalyserNode,
        data_buffer: Vec<u8>,
        fft_size: usize,
        // Kept alive — dropping stops the stream.
        _audio_ctx: AudioContext,
        _source: MediaStreamAudioSourceNode,
        _stream: MediaStream,
    }

    impl WasmAudio {
        /// Request microphone access and create the Web Audio processing chain.
        ///
        /// Must be called from within an async context triggered by a user gesture
        /// (browsers require this for `getUserMedia`).
        pub async fn new() -> Option<Self> {
            let window = web_sys::window()?;
            let navigator = window.navigator();
            let media_devices: MediaDevices = navigator.media_devices().ok()?;

            // Request microphone — needs user gesture.
            let constraints = MediaStreamConstraints::new();
            constraints.set_audio(&JsValue::from(true));

            let promise = media_devices
                .get_user_media_with_constraints(&constraints)
                .ok()?;
            let stream: MediaStream = wasm_bindgen_futures::JsFuture::from(promise)
                .await
                .ok()?
                .dyn_into()
                .ok()?;

            let audio_ctx = AudioContext::new().ok()?;
            let sample_rate = audio_ctx.sample_rate() as u32;

            let analyser = audio_ctx.create_analyser().ok()?;
            let fft_size = 2048;
            analyser.set_fft_size(fft_size);
            // No smoothing — we want raw waveform for pitch detection.
            analyser.set_smoothing_time_constant(0.0);

            let source = audio_ctx.create_media_stream_source(&stream).ok()?;
            source.connect_with_audio_node(&analyser).ok()?;
            // Do NOT connect to destination — avoids feedback.

            let data_buffer = vec![0u8; fft_size as usize];

            log::info!("Wasm audio opened: {sample_rate} Hz, FFT {fft_size}");

            Some(Self {
                sample_rate,
                analyser,
                data_buffer,
                fft_size: fft_size as usize,
                _audio_ctx: audio_ctx,
                _source: source,
                _stream: stream,
            })
        }
    }

    impl AudioCapture for WasmAudio {
        fn sample_rate(&self) -> u32 {
            self.sample_rate
        }

        fn read_samples(&mut self, buf: &mut [f32]) -> usize {
            // Ensure buffer is sized to current FFT.
            let fft = self.analyser.fft_size() as usize;
            if self.data_buffer.len() < fft {
                self.data_buffer.resize(fft, 0);
                self.fft_size = fft;
            }

            self.analyser
                .get_byte_time_domain_data(&mut self.data_buffer);

            let n = buf.len().min(self.fft_size);
            for i in 0..n {
                // Byte time-domain data is u8 [0, 255] centred on 128.
                buf[i] = (self.data_buffer[i] as f32 - 128.0) / 128.0;
            }
            n
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

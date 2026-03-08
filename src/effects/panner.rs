use std::ffi::c_void;

use miniaudio_sys::*;
use thiserror::Error;

#[derive(Debug, Error)]
#[must_use]
pub enum AudioPannerError {
    #[error("Initialization failed with error code: {0}")]
    InitializationFailed(i32), // Holds the error code from miniaudio
    #[error("Invalid number of channels: {0}")]
    InvalidChannels(usize), // Holds the invalid channel count
    #[error("Processing failed with error code: {0}")]
    ProcessFailed(i32), // Holds the error code from processing
    #[error("Buffer size mismatch: expected {0}, got {1}")]
    BufferSizeMismatch(usize, usize), // Holds the expected and actual buffer sizes
    #[error("{0}")]
    #[allow(dead_code)]
    Other(Box<dyn std::error::Error + Send + Sync + 'static>), // Wraps other errors
}

impl AudioPannerError {
    #[allow(dead_code)]
    pub fn from_other<E: std::error::Error + Send + Sync + 'static>(error: E) -> Self {
        AudioPannerError::Other(Box::new(error))
    }
}

#[derive(Debug, Clone)]
pub struct AudioPanner {
    pub instance: Box<ma_panner>,
    pub channels: usize,
    pub pan: f32,
}

impl AudioPanner {
    pub fn new(channels: usize) -> Result<Self, AudioPannerError> {
        if channels < 1 || channels > 8 {
            return Err(AudioPannerError::InvalidChannels(channels));
        }

        // SAFETY: This function is safe because it initializes the audio panner with the specified number of channels.
        // The code ensures that the panner is properly initialized and can be used for audio operations.
        unsafe {
            let mut panner: Box<ma_panner> = Box::default();
            let config = ma_panner_config_init(ma_format_f32, channels as u32);

            let result = ma_panner_init(&config, panner.as_mut());

            if result != MA_SUCCESS {
                // return Err(format!("Failed to initialize panner: {}", result));
                return Err(AudioPannerError::InitializationFailed(result));
            }

            Ok(AudioPanner {
                instance: panner,
                channels,
                pan: 0.0,
            })
        }
    }

    pub fn set_pan(&mut self, pan: f32) {
        // SAFETY: This function is safe because it sets the pan for the audio panner.
        // The code ensures that the panner is properly configured and can be used for audio operations.
        unsafe {
            let pan = pan.clamp(-1.0, 1.0);
            self.pan = pan;

            ma_panner_set_pan(self.instance.as_mut(), pan);
        }
    }

    pub fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<(), AudioPannerError> {
        if input.len() != output.len() {
            return Err(AudioPannerError::BufferSizeMismatch(
                input.len(),
                output.len(),
            ));
        }

        let frame_count = input.len() / self.channels;
        if frame_count == 0 {
            return Err(AudioPannerError::BufferSizeMismatch(
                input.len(),
                output.len(),
            ));
        }

        // SAFETY: This function is safe because it processes the audio data with the specified panner.
        // The code ensures that the panner is properly configured and can be used for audio operations.
        unsafe {
            let result = ma_panner_process_pcm_frames(
                self.instance.as_mut(),
                output.as_mut_ptr() as *mut c_void,
                input.as_ptr() as *mut c_void,
                frame_count as u64,
            );

            if result != MA_SUCCESS {
                return Err(AudioPannerError::ProcessFailed(result));
            }
        }

        Ok(())
    }
}

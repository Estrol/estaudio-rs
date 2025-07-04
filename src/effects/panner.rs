use std::ffi::c_void;

use miniaudio_sys::*;

use crate::utils;

#[derive(Debug, Clone)]
#[must_use]
pub enum AudioPannerError {
    InitializationFailed(i32),        // Holds the error code from miniaudio
    InvalidChannels(u32),             // Holds the invalid channel count
    ProcessFailed(i32),               // Holds the error code from processing
    BufferSizeMismatch(usize, usize), // Holds the expected and actual buffer sizes
}

impl std::fmt::Display for AudioPannerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioPannerError::InitializationFailed(code) => {
                write!(
                    f,
                    "Initialization failed with error code: {}, ({})",
                    code,
                    utils::ma_to_string_result(*code)
                )
            }
            AudioPannerError::InvalidChannels(channels) => {
                write!(f, "Invalid number of channels: {}", channels)
            }
            AudioPannerError::ProcessFailed(code) => {
                write!(
                    f,
                    "Processing failed with error code: {} ({})",
                    code,
                    utils::ma_to_string_result(*code)
                )
            }
            AudioPannerError::BufferSizeMismatch(expected, actual) => {
                write!(
                    f,
                    "Buffer size mismatch: expected {}, got {}",
                    expected, actual
                )
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioPanner {
    pub instance: Box<ma_panner>,
    pub channels: u32,
    pub pan: f32,
}

impl AudioPanner {
    pub fn new(channels: u32) -> Result<Self, AudioPannerError> {
        // SAFETY: This function is safe because it initializes the audio panner with the specified number of channels.
        // The code ensures that the panner is properly initialized and can be used for audio operations.
        unsafe {
            let mut panner: Box<ma_panner> = Box::new(std::mem::zeroed());
            let config = ma_panner_config_init(ma_format_f32, channels);

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

    pub fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        frame_count: u64,
    ) -> Result<(), AudioPannerError> {
        let expected_array_size = (frame_count * self.channels as u64) as usize;
        if input.len() < expected_array_size || output.len() < expected_array_size {
            return Err(AudioPannerError::BufferSizeMismatch(
                expected_array_size,
                input.len(),
            ));
        }

        // SAFETY: This function is safe because it processes the audio data with the specified panner.
        // The code ensures that the panner is properly configured and can be used for audio operations.
        unsafe {
            let result = ma_panner_process_pcm_frames(
                self.instance.as_mut(),
                output.as_mut_ptr() as *mut c_void,
                input.as_ptr() as *mut c_void,
                frame_count,
            );

            if result != MA_SUCCESS {
                // return Err(format!("Failed to process PCM frames: {}", result));
                return Err(AudioPannerError::ProcessFailed(result));
            }
        }

        Ok(())
    }
}

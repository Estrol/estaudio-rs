use std::ffi::c_void;

use miniaudio_sys::*;
use thiserror::Error;

use crate::math::{MathUtils, MathUtilsTrait as _};

#[derive(Debug, Error)]
#[must_use]
pub enum AudioResamplerError {
    #[error("Initialization failed with error code: {0}")]
    InitializationFailed(i32), // Holds the error code from miniaudio
    #[error("Invalid number of channels: {0}")]
    InvalidChannels(usize), // Holds the invalid channel count
    #[error("Invalid sample rate: {0}")]
    InvalidSampleRate(f32), // Holds the invalid sample rate
    #[error("Processing failed with error code: {0}")]
    ProcessFailed(i32), // Holds the error code from processing
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Resampler {
    pub instance: Option<Box<ma_resampler>>,
    pub dirty: bool,

    pub channels: usize,
    pub sample_rate: f32,
    pub target_sample_rate: f32,
}

#[allow(dead_code)]
impl Resampler {
    pub fn new_default() -> Resampler {
        Self::new(2, 44100.0).unwrap()
    }

    pub fn new(channels: usize, sample_rate: f32) -> Result<Self, AudioResamplerError> {
        if channels < 1 || channels > 8 {
            return Err(AudioResamplerError::InvalidChannels(channels));
        }

        if sample_rate < 8000.0 || sample_rate > 192000.0 {
            return Err(AudioResamplerError::InvalidSampleRate(sample_rate));
        }

        Ok(Resampler {
            instance: None,
            dirty: true,
            channels,
            sample_rate,
            target_sample_rate: sample_rate,
        })
    }

    pub fn bypass_mode(&self) -> bool {
        self.sample_rate == self.target_sample_rate
    }

    pub fn set_ratio(&mut self, ratio: f32) {
        let target_sample_rate = self.sample_rate as f32 * ratio;

        // SAFETY: This function is safe because it sets the resampling ratio for the audio resampler.
        // The code ensures that the resampler is properly configured and can be used for audio operations.
        if let Some(resampler) = &mut self.instance {
            unsafe {
                ma_resampler_set_rate(
                    resampler.as_mut(),
                    self.target_sample_rate as u32,
                    self.sample_rate as u32,
                );
            }
        }

        self.target_sample_rate = target_sample_rate;
    }

    pub fn set_target_sample_rate(&mut self, target_sample_rate: f32) {
        // SAFETY: This function is safe because it sets the target sample rate for the audio resampler.
        // The code ensures that the resampler is properly configured and can be used for audio operations.
        if let Some(resampler) = &mut self.instance {
            unsafe {
                ma_resampler_set_rate(
                    resampler.as_mut(),
                    target_sample_rate as u32,
                    self.sample_rate as u32,
                );
            }
        }

        self.target_sample_rate = target_sample_rate;
    }

    pub fn set_source_sample_rate(&mut self, sample_rate: f32) {
        // SAFETY: This function is safe because it sets the source sample rate for the audio resampler.
        // The code ensures that the resampler is properly configured and can be used for audio operations.
        if let Some(resampler) = &mut self.instance {
            unsafe {
                ma_resampler_set_rate(
                    resampler.as_mut(),
                    self.target_sample_rate as u32,
                    sample_rate as u32,
                );
            }
        }

        self.sample_rate = sample_rate;
    }

    pub fn ratio(&self) -> f32 {
        self.target_sample_rate / self.sample_rate
    }

    pub fn get_required_input(
        &self,
        output_frame_count: usize,
    ) -> Result<usize, AudioResamplerError> {
        Ok((output_frame_count as f32 * self.ratio() as f32) as usize)
    }

    pub fn get_expected_output(
        &self,
        input_frame_count: usize,
    ) -> Result<usize, AudioResamplerError> {
        Ok((input_frame_count as f32 / self.ratio() as f32) as usize)
    }

    pub fn set_channels(&mut self, channels: usize) -> Result<(), AudioResamplerError> {
        if channels < 1 || channels > 8 {
            return Err(AudioResamplerError::InvalidChannels(channels));
        }

        self.dirty = channels != self.channels;
        self.channels = channels;

        Ok(())
    }

    pub fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
    ) -> Result<usize, AudioResamplerError> {
        if self.bypass_mode() {
            let min_size = usize::min(input.len(), output.len());

            MathUtils::simd_copy(
                crate::macros::make_slice!(input, min_size, self.channels),
                crate::macros::make_slice_mut!(output, min_size, self.channels),
            );

            return Ok(min_size / self.channels as usize);
        }

        if self.instance.is_none() || self.dirty {
            let mut resampler: Box<ma_resampler> = Box::default();
            let config = unsafe {
                ma_resampler_config_init(
                    ma_format_f32,
                    self.channels as u32,
                    self.sample_rate as u32,
                    self.target_sample_rate as u32,
                    ma_resample_algorithm_linear,
                )
            };

            let result =
                unsafe { ma_resampler_init(&config, std::ptr::null(), resampler.as_mut()) };

            if result != MA_SUCCESS {
                return Err(AudioResamplerError::InitializationFailed(result));
            }

            self.instance = Some(resampler);
            self.dirty = false;

            self.set_target_sample_rate(self.target_sample_rate);
        }

        let target_slice_size =
            ((input.len() / self.channels as usize) as f32 / self.ratio()) as usize;
        if output.len() < target_slice_size {
            return Err(AudioResamplerError::ProcessFailed(-1));
        }

        unsafe {
            let Some(resampler) = self.instance.as_mut() else {
                panic!("AudioResampler instance is None");
            };

            let mut input_frame_count = input.len() as u64 / self.channels as u64;
            let mut output_frame_count = output.len() as u64 / self.channels as u64;

            let result = ma_resampler_process_pcm_frames(
                resampler.as_mut(),
                input.as_ptr() as *const c_void,
                &mut input_frame_count,
                output.as_mut_ptr() as *mut c_void,
                &mut output_frame_count,
            );

            if result != MA_SUCCESS {
                return Err(AudioResamplerError::ProcessFailed(result));
            }

            Ok(output_frame_count as usize)
        }
    }
}

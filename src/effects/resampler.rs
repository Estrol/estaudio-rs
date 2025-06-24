use std::ffi::c_void;

use miniaudio_sys::*;

#[derive(Debug, Clone)]
#[must_use]
pub enum AudioResamplerError {
    InitializationFailed(i32),        // Holds the error code from miniaudio
    InvalidChannels(u32),             // Holds the invalid channel count
    InvalidSampleRate(u32),           // Holds the invalid sample rate
    InvalidOperation(i32),            // Indicates an invalid operation
    ProcessFailed(i32),               // Holds the error code from processing
    BufferSizeMismatch(usize, usize), // Holds the expected and actual buffer sizes
}

impl std::fmt::Display for AudioResamplerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioResamplerError::InitializationFailed(code) => {
                write!(f, "Initialization failed with error code: {}", code)
            }
            AudioResamplerError::InvalidChannels(channels) => {
                write!(f, "Invalid number of channels: {}", channels)
            }
            AudioResamplerError::InvalidSampleRate(sample_rate) => {
                write!(f, "Invalid sample rate: {}", sample_rate)
            }
            AudioResamplerError::InvalidOperation(code) => {
                write!(f, "Invalid operation: {}", code)
            }
            AudioResamplerError::ProcessFailed(code) => {
                write!(f, "Processing failed with error code: {}", code)
            }
            AudioResamplerError::BufferSizeMismatch(expected, actual) => {
                write!(
                    f,
                    "Buffer size mismatch: expected {}, got {}",
                    expected, actual
                )
            }
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AudioResampler {
    pub instance: Box<ma_resampler>,
    pub channels: u32,
    pub sample_rate: u32,
    pub target_sample_rate: u32,
    pub frames_available: i64,
}

#[allow(dead_code)]
impl AudioResampler {
    pub fn new(channels: u32, sample_rate: u32) -> Result<Self, AudioResamplerError> {
        if channels < 1 || channels > 8 {
            return Err(AudioResamplerError::InvalidChannels(channels));
        }

        if sample_rate < 8000 || sample_rate > 192000 {
            return Err(AudioResamplerError::InvalidSampleRate(sample_rate));
        }

        // SAFETY: This function is safe because it initializes the audio resampler with the specified number of channels.
        // The code ensures that the resampler is properly initialized and can be used for audio operations.
        unsafe {
            let mut resampler: Box<ma_resampler> = Box::new(std::mem::zeroed());
            let config = ma_resampler_config_init(
                ma_format_f32,
                channels,
                sample_rate,
                sample_rate,
                ma_resample_algorithm_linear,
            );

            let result = ma_resampler_init(&config, std::ptr::null(), resampler.as_mut());

            if result != MA_SUCCESS {
                // return Err(format!("Failed to initialize resampler: {}", result));
                return Err(AudioResamplerError::InitializationFailed(result));
            }

            Ok(AudioResampler {
                instance: resampler,
                channels,
                sample_rate,
                target_sample_rate: sample_rate,
                frames_available: 0,
            })
        }
    }

    pub fn bypass_mode(&self) -> bool {
        self.sample_rate == self.target_sample_rate
    }

    pub fn set_ratio(&mut self, ratio: f32) {
        // SAFETY: This function is safe because it sets the resampling ratio for the audio resampler.
        // The code ensures that the resampler is properly configured and can be used for audio operations.
        unsafe {
            let target_sample_rate = (self.sample_rate as f32 * ratio) as u32;
            self.target_sample_rate = target_sample_rate;
            ma_resampler_set_rate(self.instance.as_mut(), target_sample_rate, self.sample_rate);
        }
    }

    pub fn set_target_sample_rate(&mut self, target_sample_rate: u32) {
        // SAFETY: This function is safe because it sets the target sample rate for the audio resampler.
        // The code ensures that the resampler is properly configured and can be used for audio operations.
        unsafe {
            self.target_sample_rate = target_sample_rate;
            ma_resampler_set_rate(self.instance.as_mut(), target_sample_rate, self.sample_rate);
        }
    }

    pub fn get_required_input(&self, output_frame_count: u64) -> Result<u64, AudioResamplerError> {
        // SAFETY: This function is safe because it calculates the required input frame count for the audio resampler.
        // The code ensures that the resampler is properly configured and can be used for audio operations.
        unsafe {
            let mut input_frame_count: u64 = 0;
            let result = ma_resampler_get_required_input_frame_count(
                self.instance.as_ref(),
                output_frame_count,
                &mut input_frame_count,
            );

            if result != MA_SUCCESS {
                // return Err(format!(
                //     "Failed to get required input frame count: {}",
                //     result
                // ));
                return Err(AudioResamplerError::InvalidOperation(result));
            }

            Ok(input_frame_count)
        }
    }

    pub fn get_expected_output(&self, input_frame_count: u64) -> Result<u64, AudioResamplerError> {
        // SAFETY: This function is safe because it calculates the expected output frame count for the audio resampler.
        // The code ensures that the resampler is properly configured and can be used for audio operations.
        unsafe {
            let mut output_frame_count: u64 = 0;
            let result = ma_resampler_get_expected_output_frame_count(
                self.instance.as_ref(),
                input_frame_count,
                &mut output_frame_count,
            );

            if result != MA_SUCCESS {
                // return Err(format!(
                //     "Failed to get expected output frame count: {}",
                //     result
                // ));
                return Err(AudioResamplerError::InvalidOperation(result));
            }

            Ok(output_frame_count)
        }
    }

    pub fn get_input_latency(&self) -> Result<u64, AudioResamplerError> {
        if self.bypass_mode() {
            return Ok(0);
        }

        // SAFETY: This function is safe because it calculates the input latency for the audio resampler.
        // The code ensures that the resampler is properly configured and can be used for audio operations.
        unsafe { Ok(ma_resampler_get_input_latency(self.instance.as_ref())) }
    }

    pub fn get_output_latency(&self) -> Result<u64, AudioResamplerError> {
        if self.bypass_mode() {
            return Ok(0);
        }

        // SAFETY: This function is safe because it calculates the output latency for the audio resampler.
        // The code ensures that the resampler is properly configured and can be used for audio operations.
        unsafe { Ok(ma_resampler_get_output_latency(self.instance.as_ref())) }
    }

    pub fn process(
        &mut self,
        input: &[f32],
        input_frame_count: u64,
        output: &mut [f32],
        output_frame_count: u64,
    ) -> Result<u64, AudioResamplerError> {
        if self.bypass_mode() {
            return Ok(output_frame_count);
        }

        if input.len() < input_frame_count as usize * self.channels as usize {
            // return Err("Input buffer is too small".to_string());
            return Err(AudioResamplerError::BufferSizeMismatch(
                input.len(),
                (input_frame_count * self.channels as u64) as usize,
            ));
        }

        if output.len() < output_frame_count as usize * self.channels as usize {
            // return Err("Output buffer is too small".to_string());
            return Err(AudioResamplerError::BufferSizeMismatch(
                output.len(),
                (output_frame_count * self.channels as u64) as usize,
            ));
        }

        // SAFETY: This function is safe because it processes the audio data with the specified resampler.
        // The code ensures that the resampler is properly configured and can be used for audio operations.
        unsafe {
            let mut expected_frame_count = output_frame_count;
            let mut frame_count = input_frame_count;

            let result = ma_resampler_process_pcm_frames(
                self.instance.as_mut(),
                input.as_ptr() as *const c_void,
                &mut frame_count,
                output.as_mut_ptr() as *mut c_void,
                &mut expected_frame_count,
            );

            if result != MA_SUCCESS {
                // return Err(format!("Failed to process PCM frames: {}", result));
                return Err(AudioResamplerError::ProcessFailed(result));
            }

            Ok(expected_frame_count)
        }
    }

    pub fn pre_process(
        &mut self,
        input: &[f32],
        frame_count: u64,
    ) -> Result<u64, AudioResamplerError> {
        if self.bypass_mode() {
            return Ok(frame_count);
        }

        let expected_output_size =
            (self.get_expected_output(frame_count)? * self.channels as u64) as usize;
        let mut output = vec![0.0f32; expected_output_size];

        // SAFETY: This function is safe because it processes the audio data with the specified resampler.
        // The code ensures that the resampler is properly configured and can be used for audio operations.
        unsafe {
            let mut expected_frame_count = frame_count;
            let mut frame_count = frame_count;

            let result = ma_resampler_process_pcm_frames(
                self.instance.as_mut(),
                input.as_ptr() as *const c_void,
                &mut frame_count,
                output.as_mut_ptr() as *mut c_void,
                &mut expected_frame_count,
            );

            if result != MA_SUCCESS {
                // return Err(format!("Failed to process PCM frames: {}", result));
                return Err(AudioResamplerError::ProcessFailed(result));
            }

            Ok(expected_frame_count)
        }
    }
}

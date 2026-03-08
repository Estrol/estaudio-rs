use miniaudio_sys::*;

use crate::{
    math::{MathUtils, MathUtilsTrait as _},
    utils,
};

#[derive(Debug)]
pub struct ChannelConverter {
    changed: bool,
    input_channels: usize,
    output_channels: usize,

    ma_converter: Option<Box<ma_channel_converter>>,
}

#[allow(dead_code)]
impl ChannelConverter {
    pub fn new() -> Self {
        Self {
            changed: true,
            input_channels: 2,
            output_channels: 2,
            ma_converter: None,
        }
    }

    pub fn set_input_channels(&mut self, channels: usize) {
        if self.input_channels != channels {
            self.input_channels = channels;
            self.changed = true;
        }
    }

    pub fn set_output_channels(&mut self, channels: usize) {
        if self.output_channels != channels {
            self.output_channels = channels;
            self.changed = true;
        }
    }

    pub fn get_input_channels(&self) -> usize {
        self.input_channels
    }

    pub fn get_output_channels(&self) -> usize {
        self.output_channels
    }

    pub fn process(&mut self, input: &[f32], output: &mut [f32]) {
        unsafe {
            if self.input_channels == self.output_channels {
                MathUtils::simd_copy(input, output);
                return;
            }

            let frame_count = crate::macros::frame_count_from!(input.len(), self.input_channels);
            if frame_count == 0 {
                return;
            }

            if self.changed {
                if let Some(mut converter) = self.ma_converter.take() {
                    ma_channel_converter_uninit(converter.as_mut(), std::ptr::null());
                }

                let config = ma_channel_converter_config_init(
                    ma_format_f32,
                    self.input_channels as u32,
                    std::ptr::null(),
                    self.output_channels as u32,
                    std::ptr::null(),
                    ma_channel_mix_mode_default,
                );

                let mut converter: Box<ma_channel_converter> = Box::new(std::mem::zeroed());
                let result =
                    ma_channel_converter_init(&config, std::ptr::null(), converter.as_mut());

                if result != MA_SUCCESS {
                    panic!(
                        "Failed to create ma_channel_converter: {}",
                        utils::ma_to_string_result(result)
                    );
                }

                self.ma_converter = Some(converter);
                self.changed = false;
            }

            let required_input_len =
                crate::macros::array_len_from!(frame_count, self.input_channels);
            let required_output_len =
                crate::macros::array_len_from!(frame_count, self.output_channels);

            if input.len() < required_input_len || output.len() < required_output_len {
                panic!(
                    "Input and output buffers must have at least {} and {} samples respectively",
                    required_input_len, required_output_len
                );
            }

            if let Some(converter) = &mut self.ma_converter {
                let result = ma_channel_converter_process_pcm_frames(
                    converter.as_mut(),
                    output.as_mut_ptr() as *mut std::ffi::c_void,
                    input.as_ptr() as *mut std::ffi::c_void,
                    frame_count as u64,
                );

                if result != MA_SUCCESS {
                    panic!(
                        "Failed to process channel conversion: {}",
                        utils::ma_to_string_result(result)
                    );
                }
            }
        }
    }
}

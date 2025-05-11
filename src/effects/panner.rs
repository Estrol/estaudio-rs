use std::ffi::c_void;

use miniaudio_sys::*;

pub struct AudioPanner {
    pub instance: Box<ma_panner>,
    pub channels: u32,
    pub pan: f32,
}

impl AudioPanner {
    pub fn new(channels: u32) -> Result<Self, String> {
        // SAFETY: This function is safe because it initializes the audio panner with the specified number of channels.
        // The code ensures that the panner is properly initialized and can be used for audio operations.
        unsafe {
            let mut panner: Box<ma_panner> = Box::new(std::mem::zeroed());
            let config = ma_panner_config_init(ma_format_f32, channels);

            let result = ma_panner_init(&config, panner.as_mut());

            if result != MA_SUCCESS {
                return Err(format!("Failed to initialize panner: {}", result));
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
    ) -> Result<(), String> {
        let expected_array_size = (frame_count * self.channels as u64) as usize;
        if input.len() < expected_array_size || output.len() < expected_array_size {
            return Err(format!(
                "Invalid array size: expected {}, got {}|{}",
                expected_array_size,
                input.len(),
                output.len()
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
                return Err(format!("Failed to process PCM frames: {}", result));
            }
        }

        Ok(())
    }
}

use std::ffi::c_void;

use miniaudio_sys::*;

pub struct AudioVolume {
    pub instance: Box<ma_gainer>,
    pub channels: u32,
    pub volume: f32,
}

impl AudioVolume {
    pub fn new(channels: u32) -> Result<Self, String> {
        // SAFETY: This function is safe because it initializes the audio gainer with the specified number of channels.
        // The code ensures that the gainer is properly initialized and can be used for audio operations.
        unsafe {
            let mut gainer = Box::<ma_gainer>::new_uninit();
            let config = ma_gainer_config_init(channels, 0);

            let result = ma_gainer_init(&config, std::ptr::null(), gainer.as_mut_ptr());

            if result != MA_SUCCESS {
                return Err(format!("Failed to initialize gainer: {}", result));
            }

            let gainer = gainer.assume_init();
            let mut instance = Self {
                instance: gainer,
                channels,
                volume: 1.0,
            };

            instance.set_volume(1.0);

            Ok(instance)
        }
    }

    pub fn set_volume(&mut self, volume: f32) {
        // SAFETY: This function is safe because it sets the gain for the audio gainer.
        // The code ensures that the gainer is properly configured and can be used for audio operations.
        unsafe {
            let gain = volume.clamp(0.0, 1.0);
            self.volume = gain;

            ma_gainer_set_master_volume(self.instance.as_mut(), gain);
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

        // SAFETY: This function is safe because it processes the audio data with the specified gainer.
        unsafe {
            let result = ma_gainer_process_pcm_frames(
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

impl Drop for AudioVolume {
    fn drop(&mut self) {
        // SAFETY: This function is safe because it properly uninitializes the audio gainer.
        // The code ensures that all resources are released and cleaned up.
        unsafe {
            ma_gainer_uninit(self.instance.as_mut(), std::ptr::null_mut());
        }
    }
}

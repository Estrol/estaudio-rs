use miniaudio_sys::*;

use crate::device::AudioDevice;

use super::spartilization_listener::AudioSpatializationListener;

pub struct AudioSpatialization {
    pub spatialization: Box<ma_spatializer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttenuationModel {
    None = 0,
    Inverse = 1,
    Linear = 2,
    Exponential = 3,
}

impl From<i32> for AttenuationModel {
    fn from(value: i32) -> Self {
        match value {
            0 => AttenuationModel::None,
            1 => AttenuationModel::Inverse,
            2 => AttenuationModel::Linear,
            3 => AttenuationModel::Exponential,
            _ => panic!("Invalid value for AttenuationModel"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Positioning {
    Absolute = 0,
    Relative = 1,
}

impl From<i32> for Positioning {
    fn from(value: i32) -> Self {
        match value {
            0 => Positioning::Absolute,
            1 => Positioning::Relative,
            _ => panic!("Invalid value for Positioning"),
        }
    }
}

impl AudioSpatialization {
    pub fn new(channels_in: u32, channels_out: u32) -> Result<Self, String> {
        unsafe {
            let mut spatializer = Box::<ma_spatializer>::new_uninit();
            let config = ma_spatializer_config_init(channels_in, channels_out);

            let result =
                ma_spatializer_init(&config, std::ptr::null_mut(), spatializer.as_mut_ptr());

            if result != 0 {
                return Err(format!("Failed to initialize spatializer: {}", result));
            }

            let spatializer = spatializer.assume_init();

            Ok(AudioSpatialization {
                spatialization: spatializer,
            })
        }
    }

    pub fn process(
        &mut self,
        listener: &mut AudioSpatializationListener,
        input: &[f32],
        output: &mut [f32],
        frame_count: u64,
    ) -> Result<(), String> {
        unsafe {
            let result = ma_spatializer_process_pcm_frames(
                self.spatialization.as_mut(),
                listener.spatialization.as_mut(),
                output.as_mut_ptr() as *mut std::ffi::c_void,
                input.as_ptr() as *const std::ffi::c_void,
                frame_count,
            );

            if result != 0 {
                return Err(format!("Failed to process spatialization: {}", result));
            }

            Ok(())
        }
    }

    pub fn set_master_volume(&mut self, volume: f32) -> Result<(), String> {
        unsafe {
            let result = ma_spatializer_set_master_volume(self.spatialization.as_mut(), volume);
            if result != 0 {
                return Err(format!("Failed to set master volume: {}", result));
            }
            Ok(())
        }
    }

    pub fn get_master_volume(&self) -> Result<f32, String> {
        unsafe {
            let mut volume: f32 = 0.0;
            let result =
                ma_spatializer_get_master_volume(self.spatialization.as_ref(), &mut volume);
            if result != 0 {
                return Err(format!("Failed to get master volume: {}", result));
            }
            Ok(volume)
        }
    }

    pub fn get_input_channels(&self) -> u32 {
        unsafe { ma_spatializer_get_input_channels(self.spatialization.as_ref()) }
    }

    pub fn get_output_channels(&self) -> u32 {
        unsafe { ma_spatializer_get_output_channels(self.spatialization.as_ref()) }
    }

    pub fn set_attenuation_model(&mut self, attenuation_model: AttenuationModel) {
        unsafe {
            ma_spatializer_set_attenuation_model(
                self.spatialization.as_mut(),
                attenuation_model as i32,
            );
        }
    }

    pub fn get_attenuation_model(&self) -> AttenuationModel {
        let model = unsafe { ma_spatializer_get_attenuation_model(self.spatialization.as_ref()) };
        AttenuationModel::from(model)
    }

    pub fn set_positioning(&mut self, positioning: Positioning) {
        unsafe {
            ma_spatializer_set_positioning(self.spatialization.as_mut(), positioning as i32);
        }
    }

    pub fn get_positioning(&self) -> Positioning {
        let positioning = unsafe { ma_spatializer_get_positioning(self.spatialization.as_ref()) };
        Positioning::from(positioning)
    }

    pub fn set_rolloff(&mut self, rolloff: f32) {
        unsafe {
            ma_spatializer_set_rolloff(self.spatialization.as_mut(), rolloff);
        }
    }

    pub fn get_rolloff(&self) -> f32 {
        unsafe { ma_spatializer_get_rolloff(self.spatialization.as_ref()) }
    }

    pub fn set_min_gain(&mut self, min_gain: f32) {
        unsafe {
            ma_spatializer_set_min_gain(self.spatialization.as_mut(), min_gain);
        }
    }

    pub fn get_min_gain(&self) -> f32 {
        unsafe { ma_spatializer_get_min_gain(self.spatialization.as_ref()) }
    }

    pub fn set_max_gain(&mut self, max_gain: f32) {
        unsafe {
            ma_spatializer_set_max_gain(self.spatialization.as_mut(), max_gain);
        }
    }

    pub fn get_max_gain(&self) -> f32 {
        unsafe { ma_spatializer_get_max_gain(self.spatialization.as_ref()) }
    }

    pub fn set_min_distance(&mut self, min_distance: f32) {
        unsafe {
            ma_spatializer_set_min_distance(self.spatialization.as_mut(), min_distance);
        }
    }

    pub fn get_min_distance(&self) -> f32 {
        unsafe { ma_spatializer_get_min_distance(self.spatialization.as_ref()) }
    }

    pub fn set_max_distance(&mut self, max_distance: f32) {
        unsafe {
            ma_spatializer_set_max_distance(self.spatialization.as_mut(), max_distance);
        }
    }

    pub fn get_max_distance(&self) -> f32 {
        unsafe { ma_spatializer_get_max_distance(self.spatialization.as_ref()) }
    }

    pub fn set_cone(&mut self, inner_angle: f32, outer_angle: f32, outer_gain: f32) {
        unsafe {
            ma_spatializer_set_cone(
                self.spatialization.as_mut(),
                inner_angle,
                outer_angle,
                outer_gain,
            );
        }
    }

    pub fn get_cone(&self) -> (f32, f32, f32) {
        unsafe {
            let mut inner_angle = 0.0;
            let mut outer_angle = 0.0;
            let mut outer_gain = 0.0;
            ma_spatializer_get_cone(
                self.spatialization.as_ref(),
                &mut inner_angle,
                &mut outer_angle,
                &mut outer_gain,
            );
            (inner_angle, outer_angle, outer_gain)
        }
    }

    pub fn set_doppler_factor(&mut self, doppler_factor: f32) {
        unsafe {
            ma_spatializer_set_doppler_factor(self.spatialization.as_mut(), doppler_factor);
        }
    }

    pub fn get_doppler_factor(&self) -> f32 {
        unsafe { ma_spatializer_get_doppler_factor(self.spatialization.as_ref()) }
    }

    pub fn set_directional_attenuation_factor(&mut self, directional_attenuation_factor: f32) {
        unsafe {
            ma_spatializer_set_directional_attenuation_factor(
                self.spatialization.as_mut(),
                directional_attenuation_factor,
            );
        }
    }

    pub fn get_directional_attenuation_factor(&self) -> f32 {
        unsafe { ma_spatializer_get_directional_attenuation_factor(self.spatialization.as_ref()) }
    }

    pub fn set_position(&mut self, x: f32, y: f32, z: f32) {
        unsafe {
            ma_spatializer_set_position(self.spatialization.as_mut(), x, y, z);
        }
    }

    pub fn get_position(&self) -> (f32, f32, f32) {
        unsafe {
            let pos = ma_spatializer_get_position(self.spatialization.as_ref());
            (pos.x, pos.y, pos.z)
        }
    }

    pub fn set_direction(&mut self, x: f32, y: f32, z: f32) {
        unsafe {
            ma_spatializer_set_direction(self.spatialization.as_mut(), x, y, z);
        }
    }

    pub fn get_direction(&self) -> (f32, f32, f32) {
        unsafe {
            let dir = ma_spatializer_get_direction(self.spatialization.as_ref());
            (dir.x, dir.y, dir.z)
        }
    }

    pub fn set_velocity(&mut self, x: f32, y: f32, z: f32) {
        unsafe {
            ma_spatializer_set_velocity(self.spatialization.as_mut(), x, y, z);
        }
    }

    pub fn get_velocity(&self) -> (f32, f32, f32) {
        unsafe {
            let vel = ma_spatializer_get_velocity(self.spatialization.as_ref());
            (vel.x, vel.y, vel.z)
        }
    }

    pub fn get_relative_position_and_direction(
        &self,
        listener: &AudioSpatializationListener,
    ) -> ((f32, f32, f32), (f32, f32, f32)) {
        unsafe {
            let mut relative_pos = ma_vec3f::default();
            let mut relative_dir = ma_vec3f::default();
            ma_spatializer_get_relative_position_and_direction(
                self.spatialization.as_ref(),
                listener.spatialization.as_ref(),
                &mut relative_pos,
                &mut relative_dir,
            );
            (
                (relative_pos.x, relative_pos.y, relative_pos.z),
                (relative_dir.x, relative_dir.y, relative_dir.z),
            )
        }
    }
}

impl Drop for AudioSpatialization {
    fn drop(&mut self) {
        unsafe {
            ma_spatializer_uninit(self.spatialization.as_mut(), std::ptr::null_mut());
        }
    }
}

pub trait AudioSpatializationHandler {
    fn set_position(&mut self, x: f32, y: f32, z: f32) -> Result<(), String>;
    fn get_position(&self) -> Result<(f32, f32, f32), String>;
    fn set_velocity(&mut self, x: f32, y: f32, z: f32) -> Result<(), String>;
    fn get_velocity(&self) -> Result<(f32, f32, f32), String>;
    fn set_direction(&mut self, x: f32, y: f32, z: f32) -> Result<(), String>;
    fn get_direction(&self) -> Result<(f32, f32, f32), String>;
    fn set_doppler_factor(&mut self, doppler_factor: f32) -> Result<(), String>;
    fn get_doppler_factor(&self) -> Result<f32, String>;
    fn set_attenuation_model(&mut self, attenuation_model: AttenuationModel) -> Result<(), String>;
    fn get_attenuation_model(&self) -> Result<AttenuationModel, String>;
    fn set_positioning(&mut self, positioning: Positioning) -> Result<(), String>;
    fn get_positioning(&self) -> Result<Positioning, String>;
    fn set_rolloff(&mut self, rolloff: f32) -> Result<(), String>;
    fn get_rolloff(&self) -> Result<f32, String>;
    fn set_min_gain(&mut self, min_gain: f32) -> Result<(), String>;
    fn get_min_gain(&self) -> Result<f32, String>;
    fn set_max_gain(&mut self, max_gain: f32) -> Result<(), String>;
    fn get_max_gain(&self) -> Result<f32, String>;
    fn set_min_distance(&mut self, min_distance: f32) -> Result<(), String>;
    fn get_min_distance(&self) -> Result<f32, String>;
    fn set_max_distance(&mut self, max_distance: f32) -> Result<(), String>;
    fn get_max_distance(&self) -> Result<f32, String>;
    fn set_cone(
        &mut self,
        inner_angle: f32,
        outer_angle: f32,
        outer_gain: f32,
    ) -> Result<(), String>;
    fn get_cone(&self) -> Result<(f32, f32, f32), String>;
    fn set_directional_attenuation_factor(
        &mut self,
        directional_attenuation_factor: f32,
    ) -> Result<(), String>;
    fn get_directional_attenuation_factor(&self) -> Result<f32, String>;
    fn get_relative_position_and_direction(
        &self,
        listener: &AudioDevice,
    ) -> Result<((f32, f32, f32), (f32, f32, f32)), String>;
}

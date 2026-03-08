#![allow(dead_code)]

use miniaudio_sys::*;
use thiserror::Error;

use crate::{device::Device, math::Vector3, utils};

use super::spartilization_listener::SpatializationListener;

#[derive(Debug, Error)]
pub enum SpatializationError {
    #[error("Initialization failed with error code: {}, {}", .0, self.get_ma_error().unwrap_or("Unknown error"))]
    InitializationFailed(i32), // Holds the error code from miniaudio
    #[error("Invalid number of channels: {0}")]
    InvalidChannels(usize), // Holds the invalid channel count
    #[error("Failed to process spatialization with error code: {0}")]
    ProcessError(i32), // Holds a custom error message for processing errors
    #[error("Operation error with error code: {0}")]
    OperationError(i32), // Holds a custom error message for general operation errors
    #[error("Instance not initialized")]
    NotInitialized,
    #[error("{0}")]
    Other(Box<dyn std::error::Error + Send + 'static>),
}

impl SpatializationError {
    pub fn get_ma_error(&self) -> Option<&str> {
        match self {
            SpatializationError::InitializationFailed(code)
            | SpatializationError::ProcessError(code)
            | SpatializationError::OperationError(code) => {
                Some(utils::ma_to_string_result(*code))
            }
            _ => None,
        }
    }

    pub fn from_other<E: std::error::Error + Send + 'static>(error: E) -> Self {
        SpatializationError::Other(Box::new(error))
    }
}

#[derive(Debug)]
pub struct Spatialization {
    pub handle: Box<ma_spatializer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
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
#[repr(C)]
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

#[allow(dead_code)]
impl Spatialization {
    pub fn new(channels_in: usize, channels_out: usize) -> Result<Self, SpatializationError> {
        if channels_in < 1 || channels_in > 8 {
            return Err(SpatializationError::InvalidChannels(channels_in));
        }

        if channels_out < 1 || channels_out > 8 {
            return Err(SpatializationError::InvalidChannels(channels_out));
        }

        unsafe {
            let mut spatializer = Box::<ma_spatializer>::new_uninit();
            let config = ma_spatializer_config_init(
                channels_in as u32, 
                channels_out as u32);

            let result = ma_spatializer_init(
                &config, 
                std::ptr::null_mut(), 
                spatializer.as_mut_ptr());

            if result != 0 {
                return Err(SpatializationError::InitializationFailed(result));
            }

            let handle = spatializer.assume_init();

            Ok(Spatialization {
                handle,
            })
        }
    }

    pub fn process(
        &mut self,
        listener: &mut SpatializationListener,
        input: &[f32],
        output: &mut [f32],
    ) -> Result<(), SpatializationError> {
        let min_length = input.len().min(output.len());
        let frame_count = crate::macros::frame_count_from!(min_length, self.get_input_channels());

        let required_input_len =
            crate::macros::array_len_from!(frame_count, self.get_input_channels());
        let required_output_len =
            crate::macros::array_len_from!(frame_count, self.get_output_channels());

        if input.len() < required_input_len || output.len() < required_output_len {
            return Err(SpatializationError::ProcessError(-2));
        }

        unsafe {
            let result = ma_spatializer_process_pcm_frames(
                self.handle.as_mut(),
                listener.handle.as_mut(),
                output.as_mut_ptr() as *mut std::ffi::c_void,
                input.as_ptr() as *const std::ffi::c_void,
                frame_count as u64,
            );

            if result != 0 {
                return Err(SpatializationError::ProcessError(result));
            }

            Ok(())
        }
    }

    pub fn set_master_volume(&mut self, volume: f32) -> Result<(), SpatializationError> {
        unsafe {
            let result = ma_spatializer_set_master_volume(self.handle.as_mut(), volume);
            if result != 0 {
                return Err(SpatializationError::OperationError(result));
            }
            Ok(())
        }
    }

    pub fn get_master_volume(&self) -> Result<f32, SpatializationError> {
        unsafe {
            let mut volume: f32 = 0.0;
            let result = ma_spatializer_get_master_volume(
                self.handle.as_ref(), 
                &mut volume);
            
            if result != 0 {
                return Err(SpatializationError::OperationError(result));
            }
            Ok(volume)
        }
    }

    pub fn get_input_channels(&self) -> u32 {
        unsafe { ma_spatializer_get_input_channels(self.handle.as_ref()) }
    }

    pub fn get_output_channels(&self) -> u32 {
        unsafe { ma_spatializer_get_output_channels(self.handle.as_ref()) }
    }

    pub fn set_attenuation_model(&mut self, attenuation_model: AttenuationModel) {
        unsafe {
            ma_spatializer_set_attenuation_model(
                self.handle.as_mut(),
                attenuation_model as i32,
            );
        }
    }

    pub fn get_attenuation_model(&self) -> AttenuationModel {
        let model = unsafe { ma_spatializer_get_attenuation_model(self.handle.as_ref()) };
        AttenuationModel::from(model)
    }

    pub fn set_positioning(&mut self, positioning: Positioning) {
        unsafe {
            ma_spatializer_set_positioning(self.handle.as_mut(), positioning as i32);
        }
    }

    pub fn get_positioning(&self) -> Positioning {
        let positioning = unsafe { ma_spatializer_get_positioning(self.handle.as_ref()) };
        Positioning::from(positioning)
    }

    pub fn set_rolloff(&mut self, rolloff: f32) {
        unsafe {
            ma_spatializer_set_rolloff(self.handle.as_mut(), rolloff);
        }
    }

    pub fn get_rolloff(&self) -> f32 {
        unsafe { ma_spatializer_get_rolloff(self.handle.as_ref()) }
    }

    pub fn set_min_gain(&mut self, min_gain: f32) {
        unsafe {
            ma_spatializer_set_min_gain(self.handle.as_mut(), min_gain);
        }
    }

    pub fn get_min_gain(&self) -> f32 {
        unsafe { ma_spatializer_get_min_gain(self.handle.as_ref()) }
    }

    pub fn set_max_gain(&mut self, max_gain: f32) {
        unsafe {
            ma_spatializer_set_max_gain(self.handle.as_mut(), max_gain);
        }
    }

    pub fn get_max_gain(&self) -> f32 {
        unsafe { ma_spatializer_get_max_gain(self.handle.as_ref()) }
    }

    pub fn set_min_distance(&mut self, min_distance: f32) {
        unsafe {
            ma_spatializer_set_min_distance(self.handle.as_mut(), min_distance);
        }
    }

    pub fn get_min_distance(&self) -> f32 {
        unsafe { ma_spatializer_get_min_distance(self.handle.as_ref()) }
    }

    pub fn set_max_distance(&mut self, max_distance: f32) {
        unsafe {
            ma_spatializer_set_max_distance(self.handle.as_mut(), max_distance);
        }
    }

    pub fn get_max_distance(&self) -> f32 {
        unsafe { ma_spatializer_get_max_distance(self.handle.as_ref()) }
    }

    pub fn set_cone(&mut self, inner_angle: f32, outer_angle: f32, outer_gain: f32) {
        unsafe {
            ma_spatializer_set_cone(
                self.handle.as_mut(),
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
                self.handle.as_ref(),
                &mut inner_angle,
                &mut outer_angle,
                &mut outer_gain,
            );
            
            (inner_angle, outer_angle, outer_gain)
        }
    }

    pub fn set_doppler_factor(&mut self, doppler_factor: f32) {
        unsafe {
            ma_spatializer_set_doppler_factor(self.handle.as_mut(), doppler_factor);
        }
    }

    pub fn get_doppler_factor(&self) -> f32 {
        unsafe { ma_spatializer_get_doppler_factor(self.handle.as_ref()) }
    }

    pub fn set_directional_attenuation_factor(&mut self, directional_attenuation_factor: f32) {
        unsafe {
            ma_spatializer_set_directional_attenuation_factor(
                self.handle.as_mut(),
                directional_attenuation_factor,
            );
        }
    }

    pub fn get_directional_attenuation_factor(&self) -> f32 {
        unsafe { ma_spatializer_get_directional_attenuation_factor(self.handle.as_ref()) }
    }

    pub fn set_position(&mut self, position: Vector3<f32>) {
        unsafe {
            ma_spatializer_set_position(self.handle.as_mut(), position.x, position.y, position.z);
        }
    }

    pub fn get_position(&self) -> Vector3<f32> {
        unsafe {
            let pos = ma_spatializer_get_position(self.handle.as_ref());
            
            Vector3::new(pos.x, pos.y, pos.z)
        }
    }

    pub fn set_direction(&mut self, position: Vector3<f32>) {
        unsafe {
            ma_spatializer_set_direction(self.handle.as_mut(), position.x, position.y, position.z);
        }
    }

    pub fn get_direction(&self) -> Vector3<f32> {
        unsafe {
            let dir = ma_spatializer_get_direction(self.handle.as_ref());
            Vector3::new(dir.x, dir.y, dir.z)
        }
    }

    pub fn set_velocity(&mut self, position: Vector3<f32>) {
        unsafe {
            ma_spatializer_set_velocity(self.handle.as_mut(), position.x, position.y, position.z);
        }
    }

    pub fn get_velocity(&self) -> Vector3<f32> {
        unsafe {
            let vel = ma_spatializer_get_velocity(self.handle.as_ref());
            Vector3::new(vel.x, vel.y, vel.z)
        }
    }

    pub fn get_relative_position_and_direction(
        &self,
        listener: &SpatializationListener,
    ) -> (Vector3<f32>, Vector3<f32>) {
        unsafe {
            let mut relative_pos = ma_vec3f::default();
            let mut relative_dir = ma_vec3f::default();
            ma_spatializer_get_relative_position_and_direction(
                self.handle.as_ref(),
                listener.handle.as_ref(),
                &mut relative_pos,
                &mut relative_dir,
            );
            (
                Vector3::new(relative_pos.x, relative_pos.y, relative_pos.z),
                Vector3::new(relative_dir.x, relative_dir.y, relative_dir.z),
            )
        }
    }
}

impl Drop for Spatialization {
    fn drop(&mut self) {
        unsafe {
            ma_spatializer_uninit(self.handle.as_mut(), std::ptr::null_mut());
        }
    }
}

/// A trait that defines methods for handling audio spatialization in 3D space.
/// This includes setting and retrieving the position, velocity, direction, and
/// other spatial properties of an audio source, as well as configuring
/// attenuation models and other related parameters.
pub trait SpatializationHandler {
    /// Set the position of the audio source in 3D space.
    fn spatial_set_position(&mut self, position: Vector3<f32>) -> Result<(), SpatializationError>;

    /// Get the position of the audio source in 3D space.
    fn spatial_get_position(&self) -> Result<Vector3<f32>, SpatializationError>;

    /// Set the velocity of the audio source in 3D space.
    fn spatial_set_velocity(&mut self, position: Vector3<f32>) -> Result<(), SpatializationError>;

    /// Get the velocity of the audio source in 3D space.
    fn spatial_get_velocity(&self) -> Result<Vector3<f32>, SpatializationError>;

    /// Set the direction of the audio source in 3D space.
    fn spatial_set_direction(&mut self, position: Vector3<f32>) -> Result<(), SpatializationError>;

    /// Get the direction of the audio source in 3D space.
    fn spatial_get_direction(&self) -> Result<Vector3<f32>, SpatializationError>;

    /// Set the Doppler factor for the audio source.
    fn spatial_set_doppler_factor(&mut self, doppler_factor: f32) -> Result<(), SpatializationError>;

    /// Get the Doppler factor of the audio source.
    fn spatial_get_doppler_factor(&self) -> Result<f32, SpatializationError>;

    /// Set the attenuation model for the audio source.
    fn spatial_set_attenuation_model(
        &mut self,
        attenuation_model: AttenuationModel,
    ) -> Result<(), SpatializationError>;

    /// Get the attenuation model of the audio source.
    fn spatial_get_attenuation_model(&self) -> Result<AttenuationModel, SpatializationError>;

    /// Set the positioning mode for the audio source.
    fn spatial_set_positioning(&mut self, positioning: Positioning)
    -> Result<(), SpatializationError>;

    /// Get the positioning mode of the audio source.
    fn spatial_get_positioning(&self) -> Result<Positioning, SpatializationError>;

    /// Set the rolloff factor for the audio source.
    fn spatial_set_rolloff(&mut self, rolloff: f32) -> Result<(), SpatializationError>;

    /// Get the rolloff factor of the audio source.
    fn spatial_get_rolloff(&self) -> Result<f32, SpatializationError>;

    /// Set the minimum gain for the audio source.
    fn spatial_set_min_gain(&mut self, min_gain: f32) -> Result<(), SpatializationError>;

    /// Get the minimum gain of the audio source.
    fn spatial_get_min_gain(&self) -> Result<f32, SpatializationError>;

    /// Set the maximum gain for the audio source.
    fn spatial_set_max_gain(&mut self, max_gain: f32) -> Result<(), SpatializationError>;

    /// Get the maximum gain of the audio source.
    fn spatial_get_max_gain(&self) -> Result<f32, SpatializationError>;

    /// Set the minimum distance for the audio source.
    fn spatial_set_min_distance(&mut self, min_distance: f32) -> Result<(), SpatializationError>;

    /// Get the minimum distance of the audio source.
    fn spatial_get_min_distance(&self) -> Result<f32, SpatializationError>;

    /// Set the maximum distance for the audio source.
    fn spatial_set_max_distance(&mut self, max_distance: f32) -> Result<(), SpatializationError>;

    /// Get the maximum distance of the audio source.
    fn spatial_get_max_distance(&self) -> Result<f32, SpatializationError>;

    /// Set the cone parameters for the audio source.
    fn spatial_set_cone(
        &mut self,
        inner_angle: f32,
        outer_angle: f32,
        outer_gain: f32,
    ) -> Result<(), SpatializationError>;

    /// Get the cone parameters of the audio source.
    fn spatial_get_cone(&self) -> Result<(f32, f32, f32), SpatializationError>;

    /// Set the directional attenuation factor for the audio source.
    fn spatial_set_directional_attenuation_factor(
        &mut self,
        directional_attenuation_factor: f32,
    ) -> Result<(), SpatializationError>;

    /// Get the directional attenuation factor of the audio source.
    fn spatial_get_directional_attenuation_factor(&self) -> Result<f32, SpatializationError>;

    /// Get the relative position and direction of the audio source with respect to a listener.
    fn spatial_get_relative_position_and_direction(
        &self,
        listener: &Device,
    ) -> Result<(Vector3<f32>, Vector3<f32>), SpatializationError>;
}

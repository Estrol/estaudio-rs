#![allow(dead_code)]

use miniaudio_sys::*;
use thiserror::Error;

use crate::{math::Vector3, utils};

#[derive(Debug, Error)]
pub enum SpatializationListenerError {
    #[error("Initialization failed with error code: {} {}", .0, self.ma_error_to_str())]
    InitializationFailed(i32), // Holds the error code from miniaudio
    #[error("Invalid number of channels: {0}")]
    InvalidChannels(u32), // Holds the invalid channel count
    #[error("Spatialization listener not initialized")]
    NotInitialized, // Indicates that the spatialization listener has not been initialized
}

impl SpatializationListenerError {
    pub fn ma_error_to_str(&self) -> &'static str {
        match self {
            SpatializationListenerError::InitializationFailed(code) => utils::ma_to_string_result(*code),
            _ => "Unknown error",
        }
    }
}

pub struct SpatializationListener {
    pub handle: Box<ma_spatializer_listener>,
}

impl SpatializationListener {
    pub fn new(channels_out: u32) -> Result<Self, SpatializationListenerError> {
        if channels_out < 1 || channels_out > 8 {
            return Err(SpatializationListenerError::InvalidChannels(
                channels_out,
            ));
        }

        unsafe {
            let mut spatializer = Box::<ma_spatializer_listener>::new_uninit();
            let config = ma_spatializer_listener_config_init(channels_out);

            let result = ma_spatializer_listener_init(
                &config,
                std::ptr::null_mut(),
                spatializer.as_mut_ptr(),
            );

            if result != 0 {
                return Err(SpatializationListenerError::InitializationFailed(
                    result,
                ));
            }

            let spatializer = spatializer.assume_init();

            Ok(SpatializationListener {
                handle: spatializer,
            })
        }
    }

    pub fn set_position(&mut self, position: Vector3<f32>) {
        unsafe {
            ma_spatializer_listener_set_position(self.handle.as_mut(), position.x, position.y, position.z);
        }
    }

    pub fn get_position(&self) -> Vector3<f32> {
        unsafe {
            let pos = ma_spatializer_listener_get_position(self.handle.as_ref());
            
            Vector3::new(pos.x, pos.y, pos.z)
        }
    }

    pub fn set_direction(&mut self, position: Vector3<f32>) {
        unsafe {
            ma_spatializer_listener_set_direction(self.handle.as_mut(), position.x, position.y, position.z);
        }
    }

    pub fn get_direction(&self) -> Vector3<f32> {
        unsafe {
            let dir = ma_spatializer_listener_get_direction(self.handle.as_ref());
            Vector3::new(dir.x, dir.y, dir.z)
        }
    }

    pub fn set_velocity(&mut self, position: Vector3<f32>) {
        unsafe {
            ma_spatializer_listener_set_velocity(self.handle.as_mut(), position.x, position.y, position.z);
        }
    }

    pub fn get_velocity(&self) -> Vector3<f32> {
        unsafe {
            let vel = ma_spatializer_listener_get_velocity(self.handle.as_ref());
            Vector3::new(vel.x, vel.y, vel.z)
        }
    }

    pub fn set_speed_of_sound(&mut self, speed: f32) {
        unsafe {
            ma_spatializer_listener_set_speed_of_sound(self.handle.as_mut(), speed);
        }
    }

    pub fn get_speed_of_sound(&self) -> f32 {
        unsafe { ma_spatializer_listener_get_speed_of_sound(self.handle.as_ref()) }
    }

    pub fn set_world_up(&mut self, position: Vector3<f32>) {
        unsafe {
            ma_spatializer_listener_set_world_up(self.handle.as_mut(), position.x, position.y, position.z);
        }
    }

    pub fn get_world_up(&self) -> Vector3<f32> {
        unsafe {
            let up = ma_spatializer_listener_get_world_up(self.handle.as_ref());
            Vector3::new(up.x, up.y, up.z)
        }
    }

    pub fn set_enabled(&mut self, is_enabled: bool) {
        unsafe {
            ma_spatializer_listener_set_enabled(
                self.handle.as_mut(),
                if is_enabled { 1 } else { 0 },
            );
        }
    }

    pub fn is_enabled(&self) -> bool {
        unsafe { ma_spatializer_listener_is_enabled(self.handle.as_ref()) != 0 }
    }

    pub fn set_cone(&mut self, inner_angle: f32, outer_angle: f32, outer_gain: f32) {
        unsafe {
            ma_spatializer_listener_set_cone(
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
            ma_spatializer_listener_get_cone(
                self.handle.as_ref(),
                &mut inner_angle,
                &mut outer_angle,
                &mut outer_gain,
            );
            
            (inner_angle, outer_angle, outer_gain)
        }
    }
}

impl Drop for SpatializationListener {
    fn drop(&mut self) {
        unsafe {
            ma_spatializer_listener_uninit(self.handle.as_mut(), std::ptr::null());
        }
    }
}

/// Trait for handling audio spatialization listener attributes.
/// This trait provides methods to set and get various attributes of the spatialization listener.
/// It is used to manage the spatialization of audio in a 3D space.
pub trait SpartialListenerHandler {
    /// Set the position of the listener in 3D space.
    fn set_position(&self, position: Vector3<f32>) -> Result<(), SpatializationListenerError>;
    /// Get the position of the listener in 3D space.
    fn get_position(&self) -> Result<Vector3<f32>, SpatializationListenerError>;
    /// Set the direction of the listener in 3D space.
    fn set_direction(&self, position: Vector3<f32>)
    -> Result<(), SpatializationListenerError>;
    /// Get the direction of the listener in 3D space.
    fn get_direction(&self) -> Result<Vector3<f32>, SpatializationListenerError>;
    /// Set the velocity of the listener in 3D space.
    fn set_velocity(&self, position: Vector3<f32>) -> Result<(), SpatializationListenerError>;
    /// Get the velocity of the listener in 3D space.
    fn get_velocity(&self) -> Result<Vector3<f32>, SpatializationListenerError>;
    /// Set the speed of sound for the listener.
    fn set_speed_of_sound(&self, speed: f32) -> Result<(), SpatializationListenerError>;
    /// Get the speed of sound for the listener.
    fn get_speed_of_sound(&self) -> Result<f32, SpatializationListenerError>;
    /// Set the world up vector for the listener.
    fn set_world_up(&self, position: Vector3<f32>) -> Result<(), SpatializationListenerError>;
    /// Get the world up vector for the listener.
    fn get_world_up(&self) -> Result<Vector3<f32>, SpatializationListenerError>;
    /// Set the cone parameters for the listener.
    fn set_cone(
        &self,
        inner_angle: f32,
        outer_angle: f32,
        outer_gain: f32,
    ) -> Result<(), SpatializationListenerError>;
    /// Get the cone parameters for the listener.
    fn get_cone(&self) -> Result<(f32, f32, f32), SpatializationListenerError>;
    /// Set whether the listener is enabled or not.
    fn set_enabled(&self, is_enabled: bool) -> Result<(), SpatializationListenerError>;
    /// Check if the listener is enabled.
    fn is_enabled(&self) -> Result<bool, SpatializationListenerError>;
}

use miniaudio_sys::*;

pub struct AudioSpatializationListener {
    pub spatialization: Box<ma_spatializer_listener>,
}

impl AudioSpatializationListener {
    pub fn new(channels_out: u32) -> Result<Self, String> {
        unsafe {
            let mut spatializer = Box::<ma_spatializer_listener>::new_uninit();
            let config = ma_spatializer_listener_config_init(channels_out);

            let result = ma_spatializer_listener_init(
                &config,
                std::ptr::null_mut(),
                spatializer.as_mut_ptr(),
            );

            if result != 0 {
                return Err(format!("Failed to initialize spatializer: {}", result));
            }

            let spatializer = spatializer.assume_init();

            Ok(AudioSpatializationListener {
                spatialization: spatializer,
            })
        }
    }

    pub fn set_position(&mut self, x: f32, y: f32, z: f32) {
        unsafe {
            ma_spatializer_listener_set_position(self.spatialization.as_mut(), x, y, z);
        }
    }

    pub fn get_position(&self) -> (f32, f32, f32) {
        unsafe {
            let pos = ma_spatializer_listener_get_position(self.spatialization.as_ref());
            (pos.x, pos.y, pos.z)
        }
    }

    pub fn set_direction(&mut self, x: f32, y: f32, z: f32) {
        unsafe {
            ma_spatializer_listener_set_direction(self.spatialization.as_mut(), x, y, z);
        }
    }

    pub fn get_direction(&self) -> (f32, f32, f32) {
        unsafe {
            let dir = ma_spatializer_listener_get_direction(self.spatialization.as_ref());
            (dir.x, dir.y, dir.z)
        }
    }

    pub fn set_velocity(&mut self, x: f32, y: f32, z: f32) {
        unsafe {
            ma_spatializer_listener_set_velocity(self.spatialization.as_mut(), x, y, z);
        }
    }

    pub fn get_velocity(&self) -> (f32, f32, f32) {
        unsafe {
            let vel = ma_spatializer_listener_get_velocity(self.spatialization.as_ref());
            (vel.x, vel.y, vel.z)
        }
    }

    pub fn set_speed_of_sound(&mut self, speed: f32) {
        unsafe {
            ma_spatializer_listener_set_speed_of_sound(self.spatialization.as_mut(), speed);
        }
    }

    pub fn get_speed_of_sound(&self) -> f32 {
        unsafe { ma_spatializer_listener_get_speed_of_sound(self.spatialization.as_ref()) }
    }

    pub fn set_world_up(&mut self, x: f32, y: f32, z: f32) {
        unsafe {
            ma_spatializer_listener_set_world_up(self.spatialization.as_mut(), x, y, z);
        }
    }

    pub fn get_world_up(&self) -> (f32, f32, f32) {
        unsafe {
            let up = ma_spatializer_listener_get_world_up(self.spatialization.as_ref());
            (up.x, up.y, up.z)
        }
    }

    pub fn set_enabled(&mut self, is_enabled: bool) {
        unsafe {
            ma_spatializer_listener_set_enabled(
                self.spatialization.as_mut(),
                if is_enabled { 1 } else { 0 },
            );
        }
    }

    pub fn is_enabled(&self) -> bool {
        unsafe { ma_spatializer_listener_is_enabled(self.spatialization.as_ref()) != 0 }
    }

    pub fn set_cone(&mut self, inner_angle: f32, outer_angle: f32, outer_gain: f32) {
        unsafe {
            ma_spatializer_listener_set_cone(
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
            ma_spatializer_listener_get_cone(
                self.spatialization.as_ref(),
                &mut inner_angle,
                &mut outer_angle,
                &mut outer_gain,
            );
            (inner_angle, outer_angle, outer_gain)
        }
    }
}

impl Drop for AudioSpatializationListener {
    fn drop(&mut self) {
        unsafe {
            ma_spatializer_listener_uninit(self.spatialization.as_mut(), std::ptr::null());
        }
    }
}

pub trait AudioSpartialListenerHandler {
    fn set_position(&self, x: f32, y: f32, z: f32) -> Result<(), String>;
    fn get_position(&self) -> Result<(f32, f32, f32), String>;
    fn set_direction(&self, x: f32, y: f32, z: f32) -> Result<(), String>;
    fn get_direction(&self) -> Result<(f32, f32, f32), String>;
    fn set_velocity(&self, x: f32, y: f32, z: f32) -> Result<(), String>;
    fn get_velocity(&self) -> Result<(f32, f32, f32), String>;
    fn set_speed_of_sound(&self, speed: f32) -> Result<(), String>;
    fn get_speed_of_sound(&self) -> Result<f32, String>;
    fn set_world_up(&self, x: f32, y: f32, z: f32) -> Result<(), String>;
    fn get_world_up(&self) -> Result<(f32, f32, f32), String>;
    fn set_cone(&self, inner_angle: f32, outer_angle: f32, outer_gain: f32) -> Result<(), String>;
    fn get_cone(&self) -> Result<(f32, f32, f32), String>;
    fn set_enabled(&self, is_enabled: bool) -> Result<(), String>;
    fn is_enabled(&self) -> Result<bool, String>;
}

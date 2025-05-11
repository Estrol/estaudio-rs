use std::sync::{Arc, Mutex};

use miniaudio_sys::*;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AudioDeviceType {
    Playback,
    Capture,
}

#[derive(Clone)]
pub struct AudioContext {
    pub(crate) context: Arc<Mutex<Box<ma_context>>>,
}

#[derive(Clone)]
pub struct AudioHardwareInfo {
    pub name: String,
    pub context: Arc<Mutex<AudioContext>>,
    pub type_: AudioDeviceType,
    pub(crate) id: ma_device_id,
}

impl AudioContext {
    pub(crate) fn new() -> Result<Self, String> {
        // SAFETY: This code is safe because it initializes the audio context and sets up the necessary configurations.
        // The code ensures that the context is properly initialized and can be used for audio operations.
        unsafe {
            let mut context = Box::<ma_context>::new_uninit();
            let result = ma_context_init(
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
                context.as_mut_ptr(),
            );

            if result != MA_SUCCESS {
                return Err(format!("Failed to initialize context: {}", result));
            }

            let context = context.assume_init();
            let context = Arc::new(Mutex::new(context));

            Ok(AudioContext { context })
        }
    }
}

pub(crate) fn enumerable(context: AudioContext) -> Result<Vec<AudioHardwareInfo>, String> {
    let mut context_lock = context.context.lock().unwrap();

    unsafe {
        let mut playback_info_array: *mut ma_device_info = std::ptr::null_mut();
        let mut playback_count = 0;

        let mut capture_info_array: *mut ma_device_info = std::ptr::null_mut();
        let mut capture_count = 0;

        let result = ma_context_get_devices(
            context_lock.as_mut(),
            &mut playback_info_array,
            &mut playback_count,
            &mut capture_info_array,
            &mut capture_count,
        );

        if result != MA_SUCCESS {
            return Err(format!("Failed to get devices: {}", result));
        }

        drop(context_lock);

        let context = Arc::new(Mutex::new(context));

        let mut devices = Vec::new();
        for i in 0..playback_count {
            let device_info = &*playback_info_array.add(i as usize);

            let name = std::ffi::CStr::from_ptr(device_info.name.as_ptr())
                .to_string_lossy()
                .into_owned();
            let id = device_info.id;

            devices.push(AudioHardwareInfo {
                name,
                id,
                type_: AudioDeviceType::Playback,
                context: Arc::clone(&context),
            });
        }

        for i in 0..capture_count {
            let device_info = &*capture_info_array.add(i as usize);

            let name = std::ffi::CStr::from_ptr(device_info.name.as_ptr())
                .to_string_lossy()
                .into_owned();
            let id: ma_device_id = device_info.id;

            devices.push(AudioHardwareInfo {
                name,
                id,
                type_: AudioDeviceType::Capture,
                context: Arc::clone(&context),
            });
        }

        Ok(devices)
    }
}

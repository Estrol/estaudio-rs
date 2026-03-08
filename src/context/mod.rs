use std::sync::Arc;

use miniaudio_sys::*;
use thiserror::Error;

use crate::utils;

#[derive(Debug, Error)]
#[must_use]
pub enum ContextError {
    #[error("Audio context initialization failed with code: {} {}", .0, self.ma_result_to_str())]
    InitializationFailed(i32),
    #[error("Audio device enumeration failed with code: {} {}", .0, self.ma_result_to_str())]
    DeviceEnumerationFailed(i32),
}

impl ContextError {
    pub fn ma_result_to_str(&self) -> &str {
        match self {
            ContextError::InitializationFailed(code)
            | ContextError::DeviceEnumerationFailed(code) => utils::ma_to_string_result(*code),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DeviceType {
    /// Playback device (output)
    #[default]
    Playback,
    /// Capture device (input)
    Capture,
    /// Duplex device (both input and output)
    Duplex,
}

#[derive(Debug)]
pub(crate) struct MaContext {
    pub context: Box<ma_context>,
}

impl MaContext {
    pub unsafe fn as_ptr(&self) -> *const ma_context {
        self.context.as_ref() as *const ma_context
    }

    pub unsafe fn as_mut_ptr(&self) -> *mut ma_context {
        // SAFETY: Each miniaudio context function has own
        // mutex, so generally it's safe to do this.
        unsafe { self.as_ptr() as *mut ma_context }
    }
}

impl MaContext {
    pub fn new(context: Box<ma_context>) -> Self {
        Self { context }
    }
}

impl Drop for MaContext {
    fn drop(&mut self) {
        unsafe {
            ma_context_uninit(self.context.as_mut());
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioHardwareInfo {
    pub name: String,
    pub is_default: bool,
    pub ty: DeviceType,

    pub(crate) id: Option<ma_device_id>,
    pub(crate) ctx: Arc<MaContext>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Backend {
    #[cfg(target_os = "windows")]
    Wasapi = ma_backend_wasapi as isize,
    #[cfg(target_os = "windows")]
    Dsound = ma_backend_dsound as isize,
    #[cfg(target_os = "windows")]
    Winmm = ma_backend_winmm as isize,
    #[cfg(target_os = "macos")]
    CoreAudio = ma_backend_coreaudio as isize,
    #[cfg(target_os = "macos")]
    Sndio = ma_backend_sndio as isize,
    #[cfg(target_os = "linux")]
    Audio4 = ma_backend_audio4 as isize,
    #[cfg(target_os = "linux")]
    Oss = ma_backend_oss as isize,
    #[cfg(target_os = "linux")]
    PulseAudio = ma_backend_pulseaudio as isize,
    #[cfg(target_os = "linux")]
    Alsa = ma_backend_alsa as isize,
    #[cfg(target_os = "linux")]
    Jack = ma_backend_jack as isize,
    #[cfg(target_os = "android")]
    AAudio = ma_backend_aaudio as isize,
    #[cfg(target_os = "android")]
    OpenSL = ma_backend_opensl as isize,
    #[cfg(target_arch = "wasm32")]
    WebAudio = ma_backend_webaudio as isize,
    Null = ma_backend_null as isize,
}

impl Into<i32> for Backend {
    fn into(self) -> i32 {
        self as i32
    }
}

#[derive(Debug, Clone)]
pub struct HardwareInfos {
    loopback_supported: bool,

    pub output: Vec<AudioHardwareInfo>,
    pub input: Vec<AudioHardwareInfo>,
}

impl HardwareInfos {
    pub fn default_output(&self) -> Option<&AudioHardwareInfo> {
        self.output.iter().find(|info| info.is_default)
    }

    pub fn default_input(&self) -> Option<&AudioHardwareInfo> {
        self.input.iter().find(|info| info.is_default)
    }

    pub fn loop_back_input(&self) -> Option<&AudioHardwareInfo> {
        if self.loopback_supported {
            self.input.iter().find(|info| info.id.is_none())
        } else {
            None
        }
    }
}

pub(crate) fn enumerable(backends: &[Backend]) -> Result<HardwareInfos, ContextError> {
    // SAFETY: As long the context is properly initialized
    // the data is always valid and the pointers are not null
    // within the *count* range.
    unsafe {
        let backends = backends
            .iter()
            .map(|b| b.clone().into())
            .collect::<Vec<_>>();

        let config = ma_context_config_init();

        let mut context: Box<ma_context> = Box::new(std::mem::zeroed());
        let result = ma_context_init(
            if backends.is_empty() {
                std::ptr::null()
            } else {
                backends.as_ptr()
            },
            backends.len() as u32,
            &config,
            context.as_mut(),
        );

        if result != MA_SUCCESS {
            return Err(ContextError::InitializationFailed(result));
        }

        let mut playback_info_array: *mut ma_device_info = std::ptr::null_mut();
        let mut playback_count = 0;

        let mut capture_info_array: *mut ma_device_info = std::ptr::null_mut();
        let mut capture_count = 0;

        let result = {
            ma_context_get_devices(
                context.as_mut(),
                &mut playback_info_array,
                &mut playback_count,
                &mut capture_info_array,
                &mut capture_count,
            )
        };

        if result != MA_SUCCESS {
            return Err(ContextError::DeviceEnumerationFailed(result));
        }

        let context = Arc::new(MaContext::new(context));

        let mut output = Vec::new();
        for i in 0..playback_count {
            let device_info = &*playback_info_array.add(i as usize);

            let name = std::ffi::CStr::from_ptr(device_info.name.as_ptr())
                .to_string_lossy()
                .into_owned();
            let id = device_info.id;
            let is_default = device_info.isDefault != 0;

            output.push(AudioHardwareInfo {
                name,
                id: Some(id),
                is_default,
                ty: DeviceType::Playback,
                ctx: Arc::clone(&context),
            });
        }

        let mut input = Vec::new();
        for i in 0..capture_count {
            let device_info = &*capture_info_array.add(i as usize);

            let name = std::ffi::CStr::from_ptr(device_info.name.as_ptr())
                .to_string_lossy()
                .into_owned();
            let id: ma_device_id = device_info.id;
            let is_default = device_info.isDefault != 0;

            input.push(AudioHardwareInfo {
                name,
                id: Some(id),
                is_default,
                ty: DeviceType::Capture,
                ctx: Arc::clone(&context),
            });
        }

        let loopback_supported = ma_context_is_loopback_supported(context.as_mut_ptr()) != 0;
        if loopback_supported {
            let loop_back_hardware = AudioHardwareInfo {
                name: "Loopback".to_string(),
                id: None,
                is_default: false,
                ty: DeviceType::Capture,
                ctx: Arc::clone(&context),
            };

            input.push(loop_back_hardware);
        }

        Ok(HardwareInfos {
            output,
            input,
            loopback_supported,
        })
    }
}

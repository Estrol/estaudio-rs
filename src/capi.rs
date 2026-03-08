use crate::{
    effects::{AttenuationModel, Positioning, SpartialListenerHandler, SpatializationHandler as _}, encoder::{EncoderSampleInfo, EncoderTrackInfo}, sample::SampleChannel
};

use super::*;

pub mod native {
    pub use crate::*;

    #[repr(C)]
    pub struct DeviceInfo {
        pub channel: usize,
        pub sample_rate: f32,
    }

    #[repr(C)]
    #[allow(dead_code)]
    pub enum SourceType {
        Path,
        Memory,
        Buffer,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    pub struct Buffer {
        pub data: *const std::os::raw::c_float,
        pub sample_rate: f32,
        pub channels: usize,
        pub frames: usize,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    pub struct Path {
        pub path: *const std::os::raw::c_char,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    pub struct Memory {
        pub data: *const std::os::raw::c_void,
        pub size: usize,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    pub union UnionSource {
        pub path: Path,
        pub memory: Memory,
        pub buffer: Buffer,
    }

    #[repr(C)]
    pub struct Source {
        pub ty: SourceType,
        pub data: UnionSource,
    }

    #[repr(C)]
    pub struct TrackInfo {
        pub source: Source,
        pub channel: usize,
        pub sample_rate: f32,
    }

    #[repr(C)]
    pub struct SampleInfo {
        pub source: Source,
        pub channel: usize,
        pub sample_rate: f32,
    }

    #[repr(C)]
    pub struct EncoderInfo {
        pub source: Source,
    }

    #[repr(C)]
    pub struct MixerInfo {
        pub channel: usize,
        pub sample_rate: f32,
    }

    #[repr(C)]
    pub struct EncoderTrackInfo {
        pub channel: usize,
        pub sample_rate: f32,
    }

    #[repr(C)]
    pub struct EncoderSampleInfo {
        pub channel: usize,
        pub sample_rate: f32,
    }

    #[repr(C)]
    pub struct Vector3 {
        pub x: f32,
        pub y: f32,
        pub z: f32,
    }

    impl Into<crate::math::Vector3<f32>> for Vector3 {
        fn into(self) -> crate::math::Vector3<f32> {
            crate::math::Vector3 {
                x: self.x,
                y: self.y,
                z: self.z,
            }
        }
    }

    impl From<crate::math::Vector3<f32>> for Vector3 {
        fn from(vec: crate::math::Vector3<f32>) -> Self {
            Vector3 {
                x: vec.x,
                y: vec.y,
                z: vec.z,
            }
        }
    }
}

macro_rules! cast_as {
    ($ptr:expr, $target:ty) => {
        unsafe { &*($ptr as *const $target) }
    };
}

macro_rules! cast_as_mut {
    ($ptr:expr, $target:ty) => {
        unsafe { &mut *($ptr as *mut $target) }
    };
}

macro_rules! ptr_write {
    ($ptr:expr, $value:expr) => {
        unsafe {
            std::ptr::write($ptr, $value);
        }
    };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_get_version() -> *const std::os::raw::c_char {
    const VERSION: &std::ffi::CStr = unsafe {
        std::ffi::CStr::from_bytes_with_nul_unchecked(
            concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes(),
        )
    };

    VERSION.as_ptr()
}

thread_local! {
    static LAST_ERROR: std::cell::RefCell<Option<std::ffi::CString>> = std::cell::RefCell::new(None);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_get_last_error() -> *const std::os::raw::c_char {
    let error = LAST_ERROR.with(|e| e.borrow().clone());
    match error {
        Some(err) => err.as_ptr(),
        None => std::ptr::null(),
    }
}

pub fn set_last_error(err: &str) {
    let c_string = std::ffi::CString::new(err)
        .unwrap_or_else(|_| std::ffi::CString::new("Unknown error").unwrap());
    LAST_ERROR.with(|e| *e.borrow_mut() = Some(c_string));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_create_playback_device(
    info: *const native::DeviceInfo,
) -> *mut Device {
    if info.is_null() {
        return std::ptr::null_mut();
    }

    let info = cast_as!(info, native::DeviceInfo);

    let device_info = DeviceInfo {
        channel: info.channel,
        sample_rate: info.sample_rate,
        ..Default::default()
    };

    match crate::create_device(device_info) {
        Ok(device) => {
            let boxed_device = Box::new(device);
            Box::into_raw(boxed_device)
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_free_playback_device(device: *mut Device) {
    if device.is_null() {
        return;
    }

    unsafe {
        let _ = Box::from_raw(device);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_device_start(device: *mut Device) -> bool {
    if device.is_null() {
        return false;
    }

    let device = cast_as_mut!(device, Device);

    match device.start() {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_device_stop(device: *mut Device) -> bool {
    if device.is_null() {
        return false;
    }

    let device = cast_as_mut!(device, Device);

    match device.stop() {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_device_set_callback(
    device: *mut Device,
    callback: Option<extern "C" fn(*const f32, *mut f32, usize)>,
) -> bool {
    if device.is_null() {
        return false;
    }

    let device = cast_as_mut!(device, Device);
    let callback = if callback.is_some() {
        Some(move |input: &[f32], output: &mut [f32]| {
            callback.unwrap()(input.as_ptr(), output.as_mut_ptr(), output.len());
        })
    } else {
        None
    };

    match device.set_callback(callback) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_device_set_input_callback(
    device: *mut Device,
    callback: Option<extern "C" fn(*const f32, usize)>,
) -> bool {
    if device.is_null() {
        return false;
    }

    let device = cast_as_mut!(device, Device);
    let callback = if callback.is_some() {
        Some(move |input: &[f32]| {
            callback.unwrap()(input.as_ptr(), input.len());
        })
    } else {
        None
    };

    match device.set_input_callback(callback) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_device_set_output_callback(
    device: *mut Device,
    callback: Option<extern "C" fn(*mut f32, usize)>,
) -> bool {
    if device.is_null() {
        return false;
    }

    let device = cast_as_mut!(device, Device);
    let callback = if callback.is_some() {
        Some(move |output: &mut [f32]| {
            callback.unwrap()(output.as_mut_ptr(), output.len());
        })
    } else {
        None
    };

    match device.set_output_callback(callback) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_device_set_attribute_f32(
    device: *mut Device,
    attr_type: native::AudioAttributes,
    value: f32,
) -> bool {
    if device.is_null() {
        return false;
    }

    let device = cast_as_mut!(device, Device);

    match device.set_attribute_f32(attr_type.into(), value) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_device_get_attribute_f32(
    device: *const Device,
    attr_type: native::AudioAttributes,
    out_value: *mut f32,
) -> bool {
    if device.is_null() || out_value.is_null() {
        return false;
    }

    let device = cast_as!(device, Device);

    match device.get_attribute_f32(attr_type.into()) {
        Ok(value) => {
            unsafe {
                *out_value = value;
            }
            true
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_device_set_attribute_bool(
    device: *mut Device,
    attr_type: native::AudioAttributes,
    value: bool,
) -> bool {
    if device.is_null() {
        return false;
    }

    let device = cast_as_mut!(device, Device);

    match device.set_attribute_bool(attr_type.into(), value) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_device_get_attribute_bool(
    device: *const Device,
    attr_type: native::AudioAttributes,
    out_value: *mut bool,
) -> bool {
    if device.is_null() || out_value.is_null() {
        return false;
    }

    let device = cast_as!(device, Device);

    match device.get_attribute_bool(attr_type.into()) {
        Ok(value) => {
            unsafe {
                *out_value = value;
            }
            true
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_device_spartial_set_position(
    device: *mut Device,
    position: native::Vector3,
) -> bool {
    if device.is_null() {
        return false;
    }

    let device = cast_as_mut!(device, Device);
    let position: crate::math::Vector3<f32> = position.into();

    match device.set_position(position) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_device_spartial_get_position(
    device: *const Device,
    out_position: *mut native::Vector3,
) -> bool {
    if device.is_null() || out_position.is_null() {
        return false;
    }

    let device = cast_as!(device, Device);

    match device.get_position() {
        Ok(position) => {
            let position: native::Vector3 = position.into();
            unsafe {
                *out_position = position;
            }
            true
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_device_spartial_set_direction(
    device: *mut Device,
    direction: native::Vector3,
) -> bool {
    if device.is_null() {
        return false;
    }

    let device = cast_as_mut!(device, Device);
    let direction: crate::math::Vector3<f32> = direction.into();

    match device.set_direction(direction) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_device_spartial_get_direction(
    device: *const Device,
    out_direction: *mut native::Vector3,
) -> bool {
    if device.is_null() || out_direction.is_null() {
        return false;
    }

    let device = cast_as!(device, Device);

    match device.get_direction() {
        Ok(direction) => {
            let direction: native::Vector3 = direction.into();
            unsafe {
                *out_direction = direction;
            }
            true
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_device_spartial_set_velocity(
    device: *mut Device,
    velocity: native::Vector3,
) -> bool {
    if device.is_null() {
        return false;
    }

    let device = cast_as_mut!(device, Device);
    let velocity: crate::math::Vector3<f32> = velocity.into();

    match device.set_velocity(velocity) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_device_spartial_get_velocity(
    device: *const Device,
    out_velocity: *mut native::Vector3,
) -> bool {
    if device.is_null() || out_velocity.is_null() {
        return false;
    }

    let device = cast_as!(device, Device);

    match device.get_velocity() {
        Ok(velocity) => {
            let velocity: native::Vector3 = velocity.into();
            unsafe {
                *out_velocity = velocity;
            }
            true
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_device_spartial_set_speed_of_sound(
    device: *mut Device,
    speed_of_sound: f32,
) -> bool {
    if device.is_null() {
        return false;
    }

    let device = cast_as_mut!(device, Device);

    match device.set_speed_of_sound(speed_of_sound) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_device_spartial_get_speed_of_sound(
    device: *const Device,
    out_speed_of_sound: *mut f32,
) -> bool {
    if device.is_null() || out_speed_of_sound.is_null() {
        return false;
    }

    let device = cast_as!(device, Device);

    match device.get_speed_of_sound() {
        Ok(speed_of_sound) => {
            unsafe {
                *out_speed_of_sound = speed_of_sound;
            }
            true
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_device_spartial_set_world_up(
    device: *mut Device,
    world_up: native::Vector3,
) -> bool {
    if device.is_null() {
        return false;
    }

    let device = cast_as_mut!(device, Device);
    let world_up: crate::math::Vector3<f32> = world_up.into();

    match device.set_world_up(world_up) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_device_spartial_get_world_up(
    device: *const Device,
    out_world_up: *mut native::Vector3,
) -> bool {
    if device.is_null() || out_world_up.is_null() {
        return false;
    }

    let device = cast_as!(device, Device);

    match device.get_world_up() {
        Ok(world_up) => {
            let world_up: native::Vector3 = world_up.into();
            unsafe {
                *out_world_up = world_up;
            }
            true
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_device_spartial_set_cone(
    device: *mut Device,
    inner_angle: f32,
    outer_angle: f32,
    outer_gain: f32,
) -> bool {
    if device.is_null() {
        return false;
    }

    let device = cast_as_mut!(device, Device);

    match device.set_cone(inner_angle, outer_angle, outer_gain) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_device_spartial_get_cone(
    device: *const Device,
    out_inner_angle: *mut f32,
    out_outer_angle: *mut f32,
    out_outer_gain: *mut f32,
) -> bool {
    if device.is_null()
        || out_inner_angle.is_null()
        || out_outer_angle.is_null()
        || out_outer_gain.is_null()
    {
        return false;
    }

    let device = cast_as!(device, Device);

    match device.get_cone() {
        Ok((inner_angle, outer_angle, outer_gain)) => {
            unsafe {
                *out_inner_angle = inner_angle;
                *out_outer_angle = outer_angle;
                *out_outer_gain = outer_gain;
            }
            true
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_device_spartial_set_enable(
    device: *mut Device,
    is_enabled: bool,
) -> bool {
    if device.is_null() {
        return false;
    }

    let device = cast_as_mut!(device, Device);

    match device.set_enabled(is_enabled) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_device_spartial_is_enabled(device: *const Device) -> bool {
    if device.is_null() {
        return false;
    }

    let device = cast_as!(device, Device);

    match device.is_enabled() {
        Ok(enabled) => enabled,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_create_track(info: *const native::TrackInfo) -> *mut Track {
    if info.is_null() {
        return std::ptr::null_mut();
    }

    let info = cast_as!(info, native::TrackInfo);

    let path_str;
    let source = match info.source.ty {
        native::SourceType::Path => {
            let c_path = unsafe { &info.source.data.path };
            if c_path.path.is_null() {
                return std::ptr::null_mut();
            }

            path_str = Some(
                unsafe { std::ffi::CStr::from_ptr(c_path.path) }
                    .to_str()
                    .unwrap_or_default()
                    .to_string(),
            );

            Source::Path(path_str.as_ref().unwrap())
        }
        native::SourceType::Memory => {
            let c_memory = unsafe { &info.source.data.memory };
            if c_memory.data.is_null() || c_memory.size == 0 {
                return std::ptr::null_mut();
            }
            let data_slice =
                unsafe { std::slice::from_raw_parts(c_memory.data as *const u8, c_memory.size) };
            Source::Memory(data_slice)
        }
        native::SourceType::Buffer => {
            let c_buffer = unsafe { &info.source.data.buffer };
            if c_buffer.data.is_null() || c_buffer.frames == 0 || c_buffer.channels == 0 {
                return std::ptr::null_mut();
            }
            let buffer_slice = unsafe {
                std::slice::from_raw_parts(
                    c_buffer.data,
                    (c_buffer.frames * c_buffer.channels) as usize,
                )
            };
            Source::Buffer(BufferInfo {
                data: buffer_slice,
                channels: c_buffer.channels,
                sample_rate: c_buffer.sample_rate,
            })
        }
    };

    let track_info = TrackInfo {
        source,
        channel: if info.channel == 0 {
            None
        } else {
            Some(info.channel)
        },
        sample_rate: if info.sample_rate == 0.0 {
            None
        } else {
            Some(info.sample_rate)
        },
    };

    match crate::create_track(track_info) {
        Ok(track) => {
            let boxed_track = Box::new(track);
            Box::into_raw(boxed_track)
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_free_track(track: *mut Track) {
    if track.is_null() {
        return;
    }

    unsafe {
        let _ = Box::from_raw(track);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_play(
    track: *mut Track,
    device: *mut Device,
) -> bool {
    if track.is_null() || device.is_null() {
        return false;
    }

    let track = cast_as_mut!(track, Track);
    let device = cast_as_mut!(device, Device);

    match track.play(device) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_stop(track: *mut Track) -> bool {
    if track.is_null() {
        return false;
    }

    let track = cast_as_mut!(track, Track);

    match track.stop() {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_is_playing(track: *const Track) -> bool {
    if track.is_null() {
        return false;
    }

    let track = cast_as!(track, Track);

    track.is_playing()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_set_start(track: *mut Track, pcm_start: usize) -> bool {
    if track.is_null() {
        return false;
    }

    let track = cast_as_mut!(track, Track);
    let value = if pcm_start == 0 {
        None
    } else {
        Some(pcm_start)
    };

    match track.set_start(value) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_set_end(track: *mut Track, pcm_end: usize) -> bool {
    if track.is_null() {
        return false;
    }

    let track = cast_as_mut!(track, Track);
    let value = if pcm_end == 0 { None } else { Some(pcm_end) };

    match track.set_end(value) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_seek(track: *mut Track, pcm_position: usize) -> bool {
    if track.is_null() {
        return false;
    }

    let track = cast_as_mut!(track, Track);

    match track.seek(pcm_position) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_get_position(track: *const Track) -> usize {
    if track.is_null() {
        return 0;
    }

    let track = cast_as!(track, Track);

    track.get_position()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_get_length(track: *const Track) -> usize {
    if track.is_null() {
        return 0;
    }

    let track = cast_as!(track, Track);

    track.get_length()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_set_looping(track: *mut Track, is_looping: bool) {
    if track.is_null() {
        return;
    }

    let track = cast_as_mut!(track, Track);
    track.set_looping(is_looping);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_is_looping(track: *const Track) -> bool {
    if track.is_null() {
        return false;
    }

    let track = cast_as!(track, Track);
    track.is_looping()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_set_attribute_f32(
    track: *mut Track,
    attr_type: native::AudioAttributes,
    value: f32,
) -> bool {
    if track.is_null() {
        return false;
    }

    let track = cast_as_mut!(track, Track);

    match track.set_attribute_f32(attr_type.into(), value) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_get_attribute_f32(
    track: *const Track,
    attr_type: native::AudioAttributes,
    out_value: *mut f32,
) -> bool {
    if track.is_null() || out_value.is_null() {
        return false;
    }

    let track = cast_as!(track, Track);

    match track.get_attribute_f32(attr_type.into()) {
        Ok(value) => {
            unsafe {
                *out_value = value;
            }
            true
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_set_attribute_bool(
    track: *mut Track,
    attr_type: native::AudioAttributes,
    value: bool,
) -> bool {
    if track.is_null() {
        return false;
    }

    let track = cast_as_mut!(track, Track);

    match track.set_attribute_bool(attr_type.into(), value) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_get_attribute_bool(
    track: *const Track,
    attr_type: native::AudioAttributes,
    out_value: *mut bool,
) -> bool {
    if track.is_null() || out_value.is_null() {
        return false;
    }

    let track = unsafe { &*track };

    match track.get_attribute_bool(attr_type.into()) {
        Ok(value) => {
            unsafe {
                *out_value = value;
            }
            true
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_spartial_set_position(
    track: *mut Track,
    position: native::Vector3,
) -> bool {
    if track.is_null() {
        return false;
    }

    let track = cast_as_mut!(track, Track);
    let position: crate::math::Vector3<f32> = position.into();

    match track.spatial_set_position(position) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_spartial_get_position(
    track: *const Track,
    out_position: *mut native::Vector3,
) -> bool {
    if track.is_null() || out_position.is_null() {
        return false;
    }

    let track = cast_as!(track, Track);

    match track.spatial_get_position() {
        Ok(position) => {
            let position: native::Vector3 = position.into();
            unsafe {
                *out_position = position;
            }
            true
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_spartial_set_velocity(
    track: *mut Track,
    velocity: native::Vector3,
) -> bool {
    if track.is_null() {
        return false;
    }

    let track = cast_as_mut!(track, Track);
    let velocity: crate::math::Vector3<f32> = velocity.into();

    match track.spatial_set_velocity(velocity) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_spartial_get_velocity(
    track: *const Track,
    out_velocity: *mut native::Vector3,
) -> bool {
    if track.is_null() || out_velocity.is_null() {
        return false;
    }

    let track = cast_as!(track, Track);

    match track.spatial_get_velocity() {
        Ok(velocity) => {
            let velocity: native::Vector3 = velocity.into();
            unsafe {
                *out_velocity = velocity;
            }
            true
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_spartial_set_direction(
    track: *mut Track,
    direction: native::Vector3,
) -> bool {
    if track.is_null() {
        return false;
    }

    let track = cast_as_mut!(track, Track);
    let direction: crate::math::Vector3<f32> = direction.into();

    match track.spatial_set_direction(direction) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_spartial_get_direction(
    track: *const Track,
    out_direction: *mut native::Vector3,
) -> bool {
    if track.is_null() || out_direction.is_null() {
        return false;
    }

    let track = cast_as!(track, Track);

    match track.spatial_get_direction() {
        Ok(direction) => {
            let direction: native::Vector3 = direction.into();
            unsafe {
                *out_direction = direction;
            }
            true
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_spartial_set_doppler_factor(
    track: *mut Track,
    doppler_factor: f32,
) -> bool {
    if track.is_null() {
        return false;
    }

    let track = cast_as_mut!(track, Track);

    match track.spatial_set_doppler_factor(doppler_factor) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_spartial_get_doppler_factor(
    track: *const Track,
    out_doppler_factor: *mut f32,
) -> bool {
    if track.is_null() || out_doppler_factor.is_null() {
        return false;
    }

    let track = cast_as!(track, Track);

    match track.spatial_get_doppler_factor() {
        Ok(doppler_factor) => {
            unsafe {
                *out_doppler_factor = doppler_factor;
            }
            true
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_spartial_set_attenuation_model(
    track: *mut Track,
    model: AttenuationModel,
) -> bool {
    if track.is_null() {
        return false;
    }

    let track = cast_as_mut!(track, Track);

    match track.spatial_set_attenuation_model(model.into()) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_spartial_get_attenuation_model(
    track: *const Track,
    out_model: *mut AttenuationModel,
) -> bool {
    if track.is_null() || out_model.is_null() {
        return false;
    }

    let track = cast_as!(track, Track);

    match track.spatial_get_attenuation_model() {
        Ok(model) => {
            unsafe {
                *out_model = model.into();
            }
            true
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_spartial_set_positioning(
    track: *mut Track,
    positioning: Positioning,
) -> bool {
    if track.is_null() {
        return false;
    }

    let track = cast_as_mut!(track, Track);

    match track.spatial_set_positioning(positioning.into()) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_spartial_get_positioning(
    track: *const Track,
    out_positioning: *mut Positioning,
) -> bool {
    if track.is_null() || out_positioning.is_null() {
        return false;
    }

    let track = cast_as!(track, Track);

    match track.spatial_get_positioning() {
        Ok(positioning) => {
            unsafe {
                *out_positioning = positioning.into();
            }
            true
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_spartial_set_rolloff(
    track: *mut Track,
    rolloff: f32,
) -> bool {
    if track.is_null() {
        return false;
    }

    let track = cast_as_mut!(track, Track);

    match track.spatial_set_rolloff(rolloff) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_spartial_get_rolloff(
    track: *const Track,
    out_rolloff: *mut f32,
) -> bool {
    if track.is_null() || out_rolloff.is_null() {
        return false;
    }

    let track = cast_as!(track, Track);

    match track.spatial_get_rolloff() {
        Ok(rolloff) => {
            unsafe {
                *out_rolloff = rolloff;
            }
            true
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_spartial_set_min_gain(
    track: *mut Track,
    min_gain: f32,
) -> bool {
    if track.is_null() {
        return false;
    }

    let track = cast_as_mut!(track, Track);

    match track.spatial_set_min_gain(min_gain) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_spartial_get_min_gain(
    track: *const Track,
    out_min_gain: *mut f32,
) -> bool {
    if track.is_null() || out_min_gain.is_null() {
        return false;
    }

    let track = cast_as!(track, Track);

    match track.spatial_get_min_gain() {
        Ok(min_gain) => {
            unsafe {
                *out_min_gain = min_gain;
            }
            true
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_spartial_set_max_gain(
    track: *mut Track,
    max_gain: f32,
) -> bool {
    if track.is_null() {
        return false;
    }

    let track = cast_as_mut!(track, Track);

    match track.spatial_set_max_gain(max_gain) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_spartial_get_max_gain(
    track: *const Track,
    out_max_gain: *mut f32,
) -> bool {
    if track.is_null() || out_max_gain.is_null() {
        return false;
    }

    let track = cast_as!(track, Track);

    match track.spatial_get_max_gain() {
        Ok(max_gain) => {
            unsafe {
                *out_max_gain = max_gain;
            }
            true
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_spartial_set_min_distance(
    track: *mut Track,
    min_distance: f32,
) -> bool {
    if track.is_null() {
        return false;
    }

    let track = cast_as_mut!(track, Track);

    match track.spatial_set_min_distance(min_distance) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_spartial_get_min_distance(
    track: *const Track,
    out_min_distance: *mut f32,
) -> bool {
    if track.is_null() || out_min_distance.is_null() {
        return false;
    }

    let track = cast_as!(track, Track);

    match track.spatial_get_min_distance() {
        Ok(min_distance) => {
            unsafe {
                *out_min_distance = min_distance;
            }
            true
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_spartial_set_max_distance(
    track: *mut Track,
    max_distance: f32,
) -> bool {
    if track.is_null() {
        return false;
    }

    let track = cast_as_mut!(track, Track);

    match track.spatial_set_max_distance(max_distance) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_spartial_get_max_distance(
    track: *const Track,
    out_max_distance: *mut f32,
) -> bool {
    if track.is_null() || out_max_distance.is_null() {
        return false;
    }

    let track = cast_as!(track, Track);

    match track.spatial_get_max_distance() {
        Ok(max_distance) => {
            unsafe {
                *out_max_distance = max_distance;
            }
            true
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_spartial_set_cone(
    track: *mut Track,
    inner_angle: f32,
    outer_angle: f32,
    outer_gain: f32,
) -> bool {
    if track.is_null() {
        return false;
    }

    let track = cast_as_mut!(track, Track);

    match track.spatial_set_cone(inner_angle, outer_angle, outer_gain) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_spartial_get_cone(
    track: *const Track,
    out_inner_angle: *mut f32,
    out_outer_angle: *mut f32,
    out_outer_gain: *mut f32,
) -> bool {
    if track.is_null()
        || out_inner_angle.is_null()
        || out_outer_angle.is_null()
        || out_outer_gain.is_null()
    {
        return false;
    }

    let track = cast_as!(track, Track);

    match track.spatial_get_cone() {
        Ok((inner_angle, outer_angle, outer_gain)) => {
            unsafe {
                *out_inner_angle = inner_angle;
                *out_outer_angle = outer_angle;
                *out_outer_gain = outer_gain;
            }
            true
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_spartial_set_directional_attenuation_factor(
    track: *mut Track,
    factor: f32,
) -> bool {
    if track.is_null() {
        return false;
    }

    let track = cast_as_mut!(track, Track);

    match track.spatial_set_directional_attenuation_factor(factor) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_spartial_get_directional_attenuation_factor(
    track: *const Track,
    out_factor: *mut f32,
) -> bool {
    if track.is_null() || out_factor.is_null() {
        return false;
    }

    let track = cast_as!(track, Track);

    match track.spatial_get_directional_attenuation_factor() {
        Ok(factor) => {
            unsafe {
                *out_factor = factor;
            }
            true
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_track_spartial_get_relative_positive_and_direction(
    track: *const Track,
    device: *const Device,
    out_relative_pos: *mut native::Vector3,
    out_direction: *mut native::Vector3,
) -> bool {
    if track.is_null() || device.is_null() || out_relative_pos.is_null() || out_direction.is_null() {
        return false;
    }

    let track = cast_as!(track, Track);
    let device = cast_as!(device, Device);

    match track.spatial_get_relative_position_and_direction(device) {
        Ok((relative_pos, direction)) => {
            let relative_pos: native::Vector3 = relative_pos.into();
            let direction: native::Vector3 = direction.into();
            unsafe {
                *out_relative_pos = relative_pos;
                *out_direction = direction;
            }
            true
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_create_sample(info: *const native::SampleInfo) -> *mut Sample {
    if info.is_null() {
        return std::ptr::null_mut();
    }

    let info = cast_as!(info, native::SampleInfo);

    let path_str;
    let source = match info.source.ty {
        native::SourceType::Path => {
            let c_path = unsafe { &info.source.data.path };
            if c_path.path.is_null() {
                return std::ptr::null_mut();
            }

            path_str = Some(
                unsafe { std::ffi::CStr::from_ptr(c_path.path) }
                    .to_str()
                    .unwrap_or_default()
                    .to_string(),
            );

            Source::Path(path_str.as_ref().unwrap())
        }
        native::SourceType::Memory => {
            let c_memory = unsafe { &info.source.data.memory };
            if c_memory.data.is_null() || c_memory.size == 0 {
                return std::ptr::null_mut();
            }
            let data_slice =
                unsafe { std::slice::from_raw_parts(c_memory.data as *const u8, c_memory.size) };
            Source::Memory(data_slice)
        }
        native::SourceType::Buffer => {
            let c_buffer = unsafe { &info.source.data.buffer };
            if c_buffer.data.is_null() || c_buffer.frames == 0 || c_buffer.channels == 0 {
                return std::ptr::null_mut();
            }
            let buffer_slice = unsafe {
                std::slice::from_raw_parts(
                    c_buffer.data,
                    (c_buffer.frames * c_buffer.channels) as usize,
                )
            };
            Source::Buffer(BufferInfo {
                data: buffer_slice,
                channels: c_buffer.channels,
                sample_rate: c_buffer.sample_rate,
            })
        }
    };

    let sample_info = SampleInfo {
        source,
        channels: if info.channel == 0 {
            None
        } else {
            Some(info.channel)
        },
        sample_rate: if info.sample_rate == 0.0 {
            None
        } else {
            Some(info.sample_rate)
        },
    };

    match crate::create_sample(sample_info) {
        Ok(sample) => {
            let boxed_sample = Box::new(sample);
            Box::into_raw(boxed_sample)
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_sample_free(sample: *mut Sample) {
    if sample.is_null() {
        return;
    }

    unsafe {
        let _ = Box::from_raw(sample);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_sample_play(
    sample: *mut Sample,
    device: *mut Device,
) -> *mut SampleChannel {
    if sample.is_null() || device.is_null() {
        return std::ptr::null_mut();
    }

    let sample = cast_as_mut!(sample, Sample);
    let device = cast_as_mut!(device, Device);

    match sample.play(device) {
        Ok(channel) => {
            let boxed_channel = Box::new(channel);
            Box::into_raw(boxed_channel)
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_sample_get_channel(sample: *mut Sample) -> *mut SampleChannel {
    if sample.is_null() {
        return std::ptr::null_mut();
    }

    let sample = cast_as_mut!(sample, Sample);
    let info = crate::sample::SampleChannelInfo {
        sample_rate: None,
        channels: None,
    };

    match sample.get_channel(Some(info)) {
        Ok(channel) => {
            let boxed_channel = Box::new(channel);
            Box::into_raw(boxed_channel)
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_sample_get_channels(
    sample: *mut Sample,
    size: usize,
    out_channels: *mut *mut SampleChannel,
) -> usize {
    if sample.is_null() || out_channels.is_null() {
        return 0;
    }

    let sample = cast_as_mut!(sample, Sample);
    let info = crate::sample::SampleChannelInfo {
        sample_rate: None,
        channels: None,
    };

    match sample.get_channels(size, Some(info)) {
        Ok(channels) => {
            let count = channels.len();
            if count == 0 {
                return 0;
            }

            let boxed_channels: Vec<Box<SampleChannel>> = channels
                .into_iter()
                .map(|channel| Box::new(channel))
                .collect();

            for (i, boxed_channel) in boxed_channels.into_iter().enumerate() {
                unsafe {
                    *out_channels.add(i) = Box::into_raw(boxed_channel);
                }
            }

            count
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            0
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_sample_channel_free(channel: *mut SampleChannel) {
    if channel.is_null() {
        return;
    }

    unsafe {
        let _ = Box::from_raw(channel);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_sample_channel_play(
    channel: *mut SampleChannel,
    device: *mut Device,
) -> bool {
    if channel.is_null() || device.is_null() {
        return false;
    }

    let channel = cast_as_mut!(channel, SampleChannel);
    let device = cast_as_mut!(device, Device);

    match channel.play(device) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_sample_channel_stop(channel: *mut SampleChannel) -> bool {
    if channel.is_null() {
        return false;
    }

    let channel = cast_as_mut!(channel, SampleChannel);

    match channel.stop() {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_sample_channel_is_finished(
    channel: *const SampleChannel,
) -> bool {
    if channel.is_null() {
        return false;
    }

    let channel = cast_as!(channel, SampleChannel);

    channel.is_finished()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_create_encoder(info: *const native::EncoderInfo) -> *mut Encoder {
    if info.is_null() {
        return std::ptr::null_mut();
    }

    let info = cast_as!(info, native::EncoderInfo);

    let path_str;
    let source = match info.source.ty {
        native::SourceType::Path => {
            let c_path = unsafe { &info.source.data.path };
            if c_path.path.is_null() {
                return std::ptr::null_mut();
            }

            path_str = Some(
                unsafe { std::ffi::CStr::from_ptr(c_path.path) }
                    .to_str()
                    .unwrap_or_default()
                    .to_string(),
            );

            Source::Path(path_str.as_ref().unwrap())
        }
        native::SourceType::Memory => {
            let c_memory = unsafe { &info.source.data.memory };
            if c_memory.data.is_null() || c_memory.size == 0 {
                return std::ptr::null_mut();
            }
            let data_slice =
                unsafe { std::slice::from_raw_parts(c_memory.data as *const u8, c_memory.size) };
            Source::Memory(data_slice)
        }
        native::SourceType::Buffer => {
            let c_buffer = unsafe { &info.source.data.buffer };
            if c_buffer.data.is_null() || c_buffer.frames == 0 || c_buffer.channels == 0 {
                return std::ptr::null_mut();
            }
            let buffer_slice = unsafe {
                std::slice::from_raw_parts(
                    c_buffer.data,
                    (c_buffer.frames * c_buffer.channels) as usize,
                )
            };
            Source::Buffer(BufferInfo {
                data: buffer_slice,
                channels: c_buffer.channels,
                sample_rate: c_buffer.sample_rate,
            })
        }
    };

    let encoder_info = EncoderInfo { source };

    match crate::create_encoder(encoder_info) {
        Ok(encoder) => {
            let boxed_encoder = Box::new(encoder);
            Box::into_raw(boxed_encoder)
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_free_encoder(encoder: *mut Encoder) {
    if encoder.is_null() {
        return;
    }

    unsafe {
        let _ = Box::from_raw(encoder);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_encoder_get_sample_rate(encoder: *const Encoder) -> f32 {
    if encoder.is_null() {
        return 0.0;
    }

    let encoder = unsafe { &*encoder };

    encoder.get_sample_rate()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_encoder_get_channel_count(encoder: *const Encoder) -> usize {
    if encoder.is_null() {
        return 0;
    }

    let encoder = cast_as!(encoder, Encoder);

    encoder.get_channel_count()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_encoder_get_data(
    encoder: *mut Encoder,
    out_data: *mut *const std::os::raw::c_float,
    out_length: *mut usize,
) -> bool {
    if encoder.is_null() || out_data.is_null() || out_length.is_null() {
        return false;
    }

    let encoder = cast_as_mut!(encoder, Encoder);

    match encoder.get_data() {
        Ok(data) => {
            unsafe {
                if !out_data.is_null() {
                    *out_data = data.as_ptr();
                }

                if !out_length.is_null() {
                    *out_length = data.len();
                }
            }
            true
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_encoder_create_track(
    encoder: *mut Encoder,
    info: *const native::EncoderTrackInfo,
) -> *mut Track {
    if encoder.is_null() {
        return std::ptr::null_mut();
    }

    let encoder = cast_as_mut!(encoder, Encoder);
    let info = if !info.is_null() {
        let info = cast_as!(info, native::EncoderTrackInfo);

        Some(EncoderTrackInfo {
            channel: if info.channel == 0 {
                None
            } else {
                Some(info.channel)
            },
            sample_rate: if info.sample_rate == 0.0 {
                None
            } else {
                Some(info.sample_rate)
            },
        })
    } else {
        None
    };

    match encoder.create_track(info) {
        Ok(track) => {
            let boxed_track = Box::new(track);
            Box::into_raw(boxed_track)
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_encoder_create_sample(
    encoder: *mut Encoder,
    info: *const native::EncoderSampleInfo,
) -> *mut Sample {
    if encoder.is_null() {
        return std::ptr::null_mut();
    }

    let encoder = cast_as_mut!(encoder, Encoder);
    let info = if !info.is_null() {
        let info = cast_as!(info, native::EncoderSampleInfo);

        Some(EncoderSampleInfo {
            channel: if info.channel == 0 {
                None
            } else {
                Some(info.channel)
            },
            sample_rate: if info.sample_rate == 0.0 {
                None
            } else {
                Some(info.sample_rate)
            },
        })
    } else {
        None
    };

    match encoder.create_sample(info) {
        Ok(sample) => {
            let boxed_sample = Box::new(sample);
            Box::into_raw(boxed_sample)
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_encoder_set_attribute_f32(
    encoder: *mut Encoder,
    attr: AudioAttributes,
    value: f32,
) -> bool {
    if encoder.is_null() {
        return false;
    }

    let encoder = cast_as_mut!(encoder, Encoder);

    match encoder.set_attribute_f32(attr, value) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_encoder_get_attribute_f32(
    encoder: *const Encoder,
    attr: AudioAttributes,
    out_value: *mut f32,
) -> bool {
    if encoder.is_null() || out_value.is_null() {
        return false;
    }

    let encoder = cast_as!(encoder, Encoder);

    match encoder.get_attribute_f32(attr) {
        Ok(value) => {
            unsafe {
                *out_value = value;
            }
            true
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_create_mixer(info: *const native::MixerInfo) -> *mut Mixer {
    if info.is_null() {
        return std::ptr::null_mut();
    }

    let info = cast_as!(info, native::MixerInfo);

    let mixer_info = MixerInfo {
        channel: info.channel,
        sample_rate: info.sample_rate,
        ..Default::default()
    };

    match crate::create_mixer(mixer_info) {
        Ok(mixer) => {
            let boxed_mixer = Box::new(mixer);
            Box::into_raw(boxed_mixer)
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_mixer_free(mixer: *mut Mixer) {
    if mixer.is_null() {
        return;
    }

    unsafe {
        let _ = Box::from_raw(mixer);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_mixer_play(
    mixer: *mut Mixer,
    device: *mut Device,
) -> bool {
    if mixer.is_null() || device.is_null() {
        return false;
    }

    let mixer = cast_as_mut!(mixer, Mixer);
    let device = cast_as_mut!(device, Device);

    match mixer.play(device) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_mixer_stop(mixer: *mut Mixer) -> bool {
    if mixer.is_null() {
        return false;
    }

    let mixer = cast_as_mut!(mixer, Mixer);

    match mixer.stop() {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_mixer_add_track(mixer: *mut Mixer, track: *mut Track) -> bool {
    if mixer.is_null() || track.is_null() {
        return false;
    }

    let mixer = cast_as_mut!(mixer, Mixer);
    let track = cast_as_mut!(track, Track);

    match mixer.add_track(&track) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_mixer_add_track_ex(
    mixer: *mut Mixer,
    track: *mut Track,
    pcm: u64,
    end: u64,
) -> bool {
    if mixer.is_null() || track.is_null() {
        return false;
    }

    let mixer = cast_as_mut!(mixer, Mixer);
    let track = cast_as_mut!(track, Track);

    let delay = if pcm == 0 { None } else { Some(pcm as usize) };
    let duration = if end == 0 { None } else { Some(end as usize) };

    match mixer.add_track_ex(&track, delay, duration) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_mixer_add_mixer(mixer: *mut Mixer, other: *mut Mixer) -> bool {
    if mixer.is_null() || other.is_null() {
        return false;
    }

    let mixer = cast_as_mut!(mixer, Mixer);
    let other = cast_as_mut!(other, Mixer);

    if mixer as *const _ == other as *const _ {
        set_last_error("Cannot add mixer to itself");
        return false;
    }

    match mixer.add_mixer(&other) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_mixer_add_mixer_ex(
    mixer: *mut Mixer,
    other: *mut Mixer,
    pcm: u64,
    end: u64,
) -> bool {
    if mixer.is_null() || other.is_null() {
        return false;
    }

    let mixer = cast_as_mut!(mixer, Mixer);
    let other = cast_as_mut!(other, Mixer);

    if mixer as *const _ == other as *const _ {
        set_last_error("Cannot add mixer to itself");
        return false;
    }

    let delay = if pcm == 0 { None } else { Some(pcm as usize) };
    let duration = if end == 0 { None } else { Some(end as usize) };

    match mixer.add_mixer_ex(&other, delay, duration) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_mixer_remove_track(mixer: *mut Mixer, track: *mut Track) -> bool {
    if mixer.is_null() || track.is_null() {
        return false;
    }

    let mixer = cast_as_mut!(mixer, Mixer);
    let track = cast_as_mut!(track, Track);

    match mixer.remove_track(&track) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_mixer_get_length(mixer: *const Mixer) -> usize {
    if mixer.is_null() {
        return 0;
    }

    let mixer = cast_as!(mixer, Mixer);

    match mixer.get_length() {
        Ok(length) => length,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            0
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_mixer_is_playing(mixer: *const Mixer) -> bool {
    if mixer.is_null() {
        return false;
    }

    let mixer = cast_as!(mixer, Mixer);

    mixer.is_playing()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_mixer_get_position(mixer: *const Mixer) -> usize {
    if mixer.is_null() {
        return 0;
    }

    let mixer = cast_as!(mixer, Mixer);

    match mixer.get_position() {
        Ok(position) => position,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            0
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_mixer_set_attribute_f32(
    mixer: *mut Mixer,
    attr: AudioAttributes,
    value: f32,
) -> bool {
    if mixer.is_null() {
        return false;
    }

    let mixer = cast_as_mut!(mixer, Mixer);

    match mixer.set_attribute_f32(attr, value) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_mixer_get_attribute_f32(
    mixer: *const Mixer,
    attr: AudioAttributes,
    out_value: *mut f32,
) -> bool {
    if mixer.is_null() || out_value.is_null() {
        return false;
    }

    let mixer = cast_as!(mixer, Mixer);

    match mixer.get_attribute_f32(attr) {
        Ok(value) => {
            unsafe {
                *out_value = value;
            }
            true
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_mixer_set_attribute_bool(
    mixer: *mut Mixer,
    attr: AudioAttributes,
    value: bool,
) -> bool {
    if mixer.is_null() {
        return false;
    }

    let mixer = cast_as_mut!(mixer, Mixer);

    match mixer.set_attribute_bool(attr, value) {
        Ok(_) => true,
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn estaudio_mixer_get_attribute_bool(
    mixer: *const Mixer,
    attr: AudioAttributes,
    out_value: *mut bool,
) -> bool {
    if mixer.is_null() || out_value.is_null() {
        return false;
    }

    let mixer = cast_as!(mixer, Mixer);

    match mixer.get_attribute_bool(attr) {
        Ok(value) => {
            unsafe {
                *out_value = value;
            }
            true
        }
        Err(e) => {
            set_last_error(&format!("{:?}", e));
            false
        }
    }
}

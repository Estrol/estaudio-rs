use std::ffi::c_char;

use crate::prelude::*;

#[unsafe(no_mangle)]
pub extern "C" fn ESTAudioEngine_new_device(channel: u32, sample_rate: u32) -> *mut AudioDevice {
    let device = est_audio::create_device(None)
        .channel(channel)
        .sample_rate(sample_rate)
        .build();

    if device.is_ok() {
        let device = device.unwrap();
        let device_ptr = Box::into_raw(Box::new(device));
        return device_ptr;
    } else {
        return std::ptr::null_mut();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ESTAUdioDevice_add_channel(
    device: *mut AudioDevice,
    channel: *mut AudioChannel,
) -> bool {
    if device.is_null() || channel.is_null() {
        return false;
    }

    let device = unsafe { &*device };
    let channel = unsafe { &*channel };

    let result = device.add_channel(channel);
    if result.is_ok() {
        return true;
    } else {
        return false;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ESTAUdioDevice_add_mixer(
    device: *mut AudioDevice,
    channel: *mut AudioMixer,
) -> bool {
    if device.is_null() || channel.is_null() {
        return false;
    }

    let device = unsafe { &*device };
    let channel = unsafe { &*channel };

    let result = device.add_mixer(channel);
    if result.is_ok() {
        return true;
    } else {
        return false;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ESTAUdioDevice_remove_channel(
    device: *mut AudioDevice,
    channel: *mut AudioChannel,
) -> bool {
    if device.is_null() || channel.is_null() {
        return false;
    }

    let device = unsafe { &*device };
    let channel = unsafe { &*channel };

    let result = device.remove_channel(channel);
    if result.is_ok() {
        return true;
    } else {
        return false;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ESTAUdioDevice_remove_channel_by_ref(
    device: *mut AudioDevice,
    channel_ref: usize,
) -> bool {
    if device.is_null() {
        return false;
    }

    let device = unsafe { &*device };

    let result = device.remove_channel_by_ref(channel_ref);
    if result.is_ok() {
        return true;
    } else {
        return false;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ESTAUdioDevice_remove_mixer(
    device: *mut AudioDevice,
    mixer: *mut AudioMixer,
) -> bool {
    if device.is_null() || mixer.is_null() {
        return false;
    }

    let device = unsafe { &*device };
    let mixer = unsafe { &*mixer };

    let result = device.remove_mixer(mixer);
    if result.is_ok() {
        return true;
    } else {
        return false;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ESTAUdioDevice_remove_mixer_by_ref(
    device: *mut AudioDevice,
    mixer_ref: u64,
) -> bool {
    if device.is_null() {
        return false;
    }

    let device = unsafe { &*device };

    let result = device.remove_mixer_by_ref(mixer_ref as usize);
    if result.is_ok() {
        return true;
    } else {
        return false;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ESTAudioDevice_free(device: *mut AudioDevice) {
    if device.is_null() {
        return;
    }

    unsafe {
        drop(Box::from_raw(device));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ESTAudioEngine_new_channel_file(
    device: *mut AudioDevice,
    file: *const i8,
) -> *mut AudioChannel {
    if file.is_null() {
        return std::ptr::null_mut();
    }

    let device = {
        if device.is_null() {
            None
        } else {
            Some(unsafe { &*device })
        }
    };

    let file = unsafe { std::ffi::CStr::from_ptr(file) };
    let file = file.to_str().unwrap_or("");

    let device = est_audio::create_channel(device).file(file).build();

    if device.is_ok() {
        let device = device.unwrap();
        let device_ptr = Box::into_raw(Box::new(device));
        return device_ptr;
    } else {
        return std::ptr::null_mut();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ESTAudioEngine_new_channel_file_buffer(
    device: *mut AudioDevice,
    buffer: *const i8,
    size: usize,
) -> *mut AudioChannel {
    if buffer.is_null() {
        return std::ptr::null_mut();
    }

    let device = {
        if device.is_null() {
            None
        } else {
            Some(unsafe { &*device })
        }
    };

    let buffer = unsafe { std::slice::from_raw_parts(buffer as *const u8, size) };

    let channel = est_audio::create_channel(device)
        .file_buffer(buffer)
        .build();

    if channel.is_ok() {
        let channel = channel.unwrap();
        let channel_ptr = Box::into_raw(Box::new(channel));
        return channel_ptr;
    } else {
        return std::ptr::null_mut();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ESTAudioEngine_new_channel_audio_buffer(
    device: *mut AudioDevice,
    channels: u32,
    sample_rate: u32,
    pcm_length: u64,
    buffer: *const i32,
) -> *mut AudioChannel {
    if buffer.is_null() {
        return std::ptr::null_mut();
    }

    let device = {
        if device.is_null() {
            None
        } else {
            Some(unsafe { &*device })
        }
    };

    let buffer = unsafe {
        std::slice::from_raw_parts(buffer as *const f32, (channels * sample_rate) as usize)
    };

    let audio_buffer_desc = AudioBufferDesc {
        channels,
        sample_rate,
        pcm_length,
        buffer,
    };

    let channel = est_audio::create_channel(device)
        .audio_buffer(audio_buffer_desc)
        .build();

    if channel.is_ok() {
        let channel = channel.unwrap();
        let channel_ptr = Box::into_raw(Box::new(channel));
        return channel_ptr;
    } else {
        return std::ptr::null_mut();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ESTAudioChannel_play(channel: *mut AudioChannel) -> bool {
    if channel.is_null() {
        return false;
    }

    let channel = unsafe { &*channel };

    let result = channel.play();
    if result.is_ok() {
        return true;
    } else {
        return false;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ESTAudioChannel_stop(channel: *mut AudioChannel) -> bool {
    if channel.is_null() {
        return false;
    }

    let channel = unsafe { &*channel };

    let result = channel.stop();
    if result.is_ok() {
        return true;
    } else {
        return false;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ESTAudioChannel_is_playing(channel: *mut AudioChannel) -> bool {
    if channel.is_null() {
        return false;
    }

    let channel = unsafe { &*channel };

    channel.is_playing()
}

#[unsafe(no_mangle)]
pub extern "C" fn ESTAudioChannel_set_attribute_f32(
    channel: *mut AudioChannel,
    attribute: *const c_char,
    value: f32,
) -> bool {
    if channel.is_null() || attribute.is_null() {
        return false;
    }

    let channel = unsafe { &*channel };
    let attribute = unsafe { std::ffi::CStr::from_ptr(attribute) };
    let attribute = attribute.to_str().unwrap_or("");

    channel
        .set_attribute_f32(AudioAttributes::from(attribute), value)
        .is_ok()
}

#[unsafe(no_mangle)]
pub extern "C" fn ESTAudioChannel_set_attribute_bool(
    channel: *mut AudioChannel,
    attribute: *const c_char,
    value: bool,
) -> bool {
    if channel.is_null() || attribute.is_null() {
        return false;
    }

    let channel = unsafe { &*channel };
    let attribute = unsafe { std::ffi::CStr::from_ptr(attribute) };
    let attribute = attribute.to_str().unwrap_or("");

    channel
        .set_attribute_bool(AudioAttributes::from(attribute), value)
        .is_ok()
}

#[unsafe(no_mangle)]
pub extern "C" fn ESTAudioChannel_free(channel: *mut AudioChannel) {
    if channel.is_null() {
        return;
    }

    unsafe {
        drop(Box::from_raw(channel));
    }
}

#![allow(unreachable_code)]
#![allow(dead_code)]

use miniaudio_sys::*;

pub enum TweenType {
    Linear,
    Quadratic,
    Cubic,
    Quartic,
    Quintic,
    Sine,
    Exponential,
    Circular,
}

#[allow(dead_code)]
pub fn tween(tween_type: TweenType, t: f32) -> f32 {
    match tween_type {
        TweenType::Linear => t,
        TweenType::Quadratic => t * t,
        TweenType::Cubic => t * t * t,
        TweenType::Quartic => t * t * t * t,
        TweenType::Quintic => t * t * t * t * t,
        TweenType::Sine => (t * std::f32::consts::PI / 2.0).sin(),
        TweenType::Exponential => {
            if t == 0.0 {
                0.0
            } else {
                2.0_f32.powf(10.0 * (t - 1.0))
            }
        }
        TweenType::Circular => (1.0 - (1.0 - t * t).sqrt()).sqrt(),
    }
}

pub fn ma_to_string_result(result: ma_result) -> &'static str {
    match result as i32 {
        MA_SUCCESS => "Success",
        MA_ERROR => "Error",
        MA_INVALID_ARGS => "Invalid arguments",
        MA_INVALID_OPERATION => "Invalid operation",
        MA_OUT_OF_MEMORY => "Out of memory",
        MA_OUT_OF_RANGE => "Out of range",
        MA_ACCESS_DENIED => "Access denied",
        MA_DOES_NOT_EXIST => "Does not exist",
        MA_ALREADY_EXISTS => "Already exists",
        MA_TOO_MANY_OPEN_FILES => "Too many open files",
        MA_INVALID_FILE => "Invalid file",
        MA_TOO_BIG => "Too big",
        MA_PATH_TOO_LONG => "Path too long",
        MA_NAME_TOO_LONG => "Name too long",
        MA_NOT_DIRECTORY => "Not a directory",
        MA_IS_DIRECTORY => "Is a directory",
        MA_DIRECTORY_NOT_EMPTY => "Directory not empty",
        MA_AT_END => "At end",
        MA_NO_SPACE => "No space",
        MA_BUSY => "Busy",
        MA_IO_ERROR => "IO error",
        MA_INTERRUPT => "Interrupted",
        MA_UNAVAILABLE => "Unavailable",
        MA_ALREADY_IN_USE => "Already in use",
        MA_BAD_ADDRESS => "Bad address",
        MA_BAD_SEEK => "Bad seek",
        MA_BAD_PIPE => "Bad pipe",
        MA_DEADLOCK => "Deadlock",
        MA_TOO_MANY_LINKS => "Too many links",
        MA_NOT_IMPLEMENTED => "Not implemented",
        MA_NO_MESSAGE => "No message",
        MA_BAD_MESSAGE => "Bad message",
        MA_NO_DATA_AVAILABLE => "No data available",
        MA_INVALID_DATA => "Invalid data",
        MA_TIMEOUT => "Timeout",
        MA_NO_NETWORK => "No network",
        MA_NOT_UNIQUE => "Not unique",
        MA_NOT_SOCKET => "Not a socket",
        MA_NO_ADDRESS => "No address",
        MA_BAD_PROTOCOL => "Bad protocol",
        MA_PROTOCOL_UNAVAILABLE => "Protocol unavailable",
        MA_PROTOCOL_NOT_SUPPORTED => "Protocol not supported",
        MA_PROTOCOL_FAMILY_NOT_SUPPORTED => "Protocol family not supported",
        MA_ADDRESS_FAMILY_NOT_SUPPORTED => "Address family not supported",
        MA_SOCKET_NOT_SUPPORTED => "Socket not supported",
        MA_CONNECTION_RESET => "Connection reset",
        MA_ALREADY_CONNECTED => "Already connected",
        MA_NOT_CONNECTED => "Not connected",
        MA_CONNECTION_REFUSED => "Connection refused",
        MA_NO_HOST => "No host",
        MA_IN_PROGRESS => "In progress",
        MA_CANCELLED => "Cancelled",
        MA_MEMORY_ALREADY_MAPPED => "Memory already mapped",
        MA_CRC_MISMATCH => "CRC mismatch",
        MA_FORMAT_NOT_SUPPORTED => "Format not supported",
        MA_DEVICE_TYPE_NOT_SUPPORTED => "Device type not supported",
        MA_SHARE_MODE_NOT_SUPPORTED => "Share mode not supported",
        MA_NO_BACKEND => "No backend",
        MA_NO_DEVICE => "No device",
        MA_API_NOT_FOUND => "API not found",
        MA_INVALID_DEVICE_CONFIG => "Invalid device configuration",
        MA_LOOP => "Loop",
        MA_BACKEND_NOT_ENABLED => "Backend not enabled",
        MA_DEVICE_NOT_INITIALIZED => "Device not initialized",
        MA_DEVICE_ALREADY_INITIALIZED => "Device already initialized",
        MA_DEVICE_NOT_STARTED => "Device not started",
        MA_DEVICE_NOT_STOPPED => "Device not stopped",
        MA_FAILED_TO_INIT_BACKEND => "Failed to initialize backend",
        MA_FAILED_TO_OPEN_BACKEND_DEVICE => "Failed to open backend device",
        MA_FAILED_TO_START_BACKEND_DEVICE => "Failed to start backend device",
        MA_FAILED_TO_STOP_BACKEND_DEVICE => "Failed to stop backend device",
        _ => "Unknown error",
    }
}

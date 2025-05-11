#![allow(unreachable_code)]

use std::sync::MutexGuard;

use miniaudio_sys::*;

pub fn array_fast_copy_f32(
    src: &[f32],
    dst: &mut [f32],
    src_offset: usize,
    dst_offset: usize,
    length: usize,
) {
    if src_offset + length > src.len() || dst_offset + length > dst.len() {
        panic!("Array copy out of bounds");
    }

    // AVX implementation
    #[cfg(target_feature = "avx")]
    {
        use std::arch::x86_64::*;
        unsafe {
            let src_ptr = src.as_ptr().add(src_offset);
            let dst_ptr = dst.as_mut_ptr().add(dst_offset);

            for i in 0..length / 8 {
                _mm256_storeu_ps(
                    dst_ptr.add(i * 8),                  // Corrected: Multiply i by 8
                    _mm256_loadu_ps(src_ptr.add(i * 8)), // Corrected: Multiply i by 8
                );
            }

            // Handle remaining elements
            for i in (length / 8) * 8..length {
                dst[dst_offset + i] = src[src_offset + i];
            }
        }
        return;
    }

    // SSE implementation
    #[cfg(all(target_feature = "sse", not(target_feature = "avx")))]
    {
        use std::arch::x86_64::*;
        unsafe {
            let src_ptr = src.as_ptr().add(src_offset);
            let dst_ptr = dst.as_mut_ptr().add(dst_offset);

            for i in 0..length / 4 {
                _mm_storeu_ps(
                    dst_ptr.add(i * 4),               // Corrected: Multiply i by 4
                    _mm_loadu_ps(src_ptr.add(i * 4)), // Corrected: Multiply i by 4
                );
            }

            // Handle remaining elements
            for i in (length / 4) * 4..length {
                dst[dst_offset + i] = src[src_offset + i];
            }
        }
        return;
    }

    // NEON implementation (for ARM)
    #[cfg(target_feature = "neon")]
    {
        use std::arch::aarch64::*;
        unsafe {
            let src_ptr = src.as_ptr().add(src_offset);
            let dst_ptr = dst.as_mut_ptr().add(dst_offset);

            for i in 0..length / 4 {
                vst1q_f32(
                    dst_ptr.add(i * 4),            // Corrected: Multiply i by 4
                    vld1q_f32(src_ptr.add(i * 4)), // Corrected: Multiply i by 4
                );
            }

            // Handle remaining elements
            for i in (length / 4) * 4..length {
                dst[dst_offset + i] = src[src_offset + i];
            }
        }
        return;
    }

    // Fallback implementation
    for i in 0..length {
        dst[dst_offset + i] = src[src_offset + i];
    }
}

pub fn array_fast_set_value_f32(arr: &mut [f32], value: f32) {
    let length = arr.len();
    let mut i = 0;

    // AVX implementation
    #[cfg(target_feature = "avx")]
    {
        use std::arch::x86_64::*;
        unsafe {
            let value_vec = _mm256_set1_ps(value);
            while i + 8 <= length {
                _mm256_storeu_ps(arr.as_mut_ptr().add(i), value_vec);
                i += 8;
            }
        }
    }

    // SSE implementation
    #[cfg(all(target_feature = "sse", not(target_feature = "avx")))]
    {
        use std::arch::x86_64::*;
        unsafe {
            let value_vec = _mm_set1_ps(value);
            while i + 4 <= length {
                _mm_storeu_ps(arr.as_mut_ptr().add(i), value_vec);
                i += 4;
            }
        }
    }

    // NEON implementation (for ARM)
    #[cfg(target_feature = "neon")]
    {
        use std::arch::aarch64::*;
        unsafe {
            let value_vec = vdupq_n_f32(value);
            while i + 4 <= length {
                vst1q_f32(arr.as_mut_ptr().add(i), value_vec);
                i += 4;
            }
        }
    }

    // Fallback implementation
    for j in i..length {
        arr[j] = value;
    }
}

pub fn array_fast_add_value_f32(src: &[f32], dst: &mut [f32], length: usize) {
    if (length > src.len()) || (length > dst.len()) {
        panic!("Array add out of bounds");
    }

    let mut i = 0;

    // AVX implementation
    #[cfg(target_feature = "avx")]
    {
        use std::arch::x86_64::*;
        unsafe {
            let src_ptr = src.as_ptr();
            let dst_ptr = dst.as_mut_ptr();

            while i + 8 <= length {
                let src_vec = _mm256_loadu_ps(src_ptr.add(i));
                let dst_vec = _mm256_loadu_ps(dst_ptr.add(i));
                _mm256_storeu_ps(dst_ptr.add(i), _mm256_add_ps(src_vec, dst_vec));
                i += 8;
            }
        }
    }

    // SSE implementation
    #[cfg(all(target_feature = "sse", not(target_feature = "avx")))]
    {
        use std::arch::x86_64::*;
        unsafe {
            let src_ptr = src.as_ptr();
            let dst_ptr = dst.as_mut_ptr();

            while i + 4 <= length {
                let src_vec = _mm_loadu_ps(src_ptr.add(i));
                let dst_vec = _mm_loadu_ps(dst_ptr.add(i));
                _mm_storeu_ps(dst_ptr.add(i), _mm_add_ps(src_vec, dst_vec));
                i += 4;
            }
        }
    }

    // NEON implementation (for ARM)
    #[cfg(target_feature = "neon")]
    {
        use std::arch::aarch64::*;
        unsafe {
            let src_ptr = src.as_ptr();
            let dst_ptr = dst.as_mut_ptr();

            while i + 4 <= length {
                let src_vec = vld1q_f32(src_ptr.add(i));
                let dst_vec = vld1q_f32(dst_ptr.add(i));
                vst1q_f32(dst_ptr.add(i), vaddq_f32(src_vec, dst_vec));
                i += 4;
            }
        }
    }

    // Fallback implementation
    for j in i..length {
        dst[j] += src[j];
    }
}

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

pub trait MutexPoison<T> {
    fn lock_poison(&self) -> MutexGuard<'_, T>;
    fn try_lock_poison(&self) -> Option<MutexGuard<'_, T>>;
}

pub struct PCMIndex {
    pub index: usize,
}

impl PCMIndex {
    pub fn new(index: usize) -> Option<Self> {
        if index > 0 {
            Some(PCMIndex { index })
        } else {
            None
        }
    }

    pub fn from_secs(seconds: f32, sample_rate: u32) -> Option<Self> {
        let index = (seconds * sample_rate as f32) as usize;
        Self::new(index)
    }

    pub fn from_millis(milliseconds: f32, sample_rate: u32) -> Option<Self> {
        let index = (milliseconds * sample_rate as f32 / 1000.0) as usize;
        Self::new(index)
    }

    pub fn to_secs(&self, sample_rate: u32) -> f32 {
        self.index as f32 / sample_rate as f32
    }

    pub fn to_millis(&self, sample_rate: u32) -> f32 {
        self.index as f32 * 1000.0 / sample_rate as f32
    }
}

pub trait IntoOptionU64 {
    fn into_option_u64(self) -> Option<u64>;
}

impl IntoOptionU64 for Option<PCMIndex> {
    fn into_option_u64(self) -> Option<u64> {
        match self {
            Some(index) => Some(index.index as u64),
            None => None,
        }
    }
}

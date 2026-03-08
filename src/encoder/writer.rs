use miniaudio_sys as ma;
use thiserror::Error;

use crate::utils;

pub enum WriteFormat {
    Wav,
}

pub struct Writer {
    writer: Box<ma::ma_encoder>,
    channels: usize,
}

impl Writer {
    pub fn new(
        path: &str,
        format: WriteFormat,
        channels: usize,
        sample_rate: f32,
    ) -> Result<Self, WriterError> {
        unsafe {
            let config = ma::ma_encoder_config_init(
                match format {
                    WriteFormat::Wav => ma::ma_encoding_format_wav,
                },
                ma::ma_format_f32,
                channels as u32,
                sample_rate as u32,
            );

            let mut encoder: Box<ma::ma_encoder> = Box::default();
            let cstring = std::ffi::CString::new(path).unwrap();
            let result = ma::ma_encoder_init_file(cstring.as_ptr(), &config, encoder.as_mut());
            if result != ma::MA_SUCCESS {
                return Err(WriterError::InitializationFailed(result));
            }

            Ok(Self {
                writer: encoder,
                channels,
            })
        }
    }

    pub fn write(&mut self, data: &[f32]) -> Result<usize, WriterError> {
        unsafe {
            let input_frames = (data.len() / self.channels as usize) as u64;
            let mut written_frames = 0;

            let result = ma::ma_encoder_write_pcm_frames(
                self.writer.as_mut(),
                data.as_ptr() as *const std::ffi::c_void,
                input_frames,
                &mut written_frames,
            );

            if result != ma::MA_SUCCESS {
                return Err(WriterError::ProcessFailed(result));
            }

            Ok(written_frames as usize)
        }
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        unsafe {
            ma::ma_encoder_uninit(self.writer.as_mut());
        }
    }
}

#[derive(Debug, Error)]
pub enum WriterError {
    #[error("Writer initialization failed with code: {} {}", .0, self.ma_result_to_str())]
    InitializationFailed(i32),
    #[error("Writing process failed with code: {} {}", .0, self.ma_result_to_str())]
    ProcessFailed(i32),
}

impl WriterError {
    pub fn ma_result_to_str(&self) -> &'static str {
        match self {
            WriterError::InitializationFailed(code) | WriterError::ProcessFailed(code) => {
                utils::ma_to_string_result(*code)
            }
        }
    }
}

use std::{os::raw::c_void, sync::Arc};

use miniaudio_sys::*;
use thiserror::Error;

use crate::utils;

pub(crate) mod cache;
pub(crate) mod ogg;

#[derive(Debug)]
pub struct AudioReader {
    pub cache: Option<Arc<cache::AudioCache>>,
    pub audio_buffer: Option<Box<ma_audio_buffer>>,

    pub sample_rate: f32,
    pub channels: usize,
    pub pcm_length: usize,
    pub position: usize,
}

impl Clone for AudioReader {
    fn clone(&self) -> Self {
        let cache_cloned = self.cache.clone();
        let buffer_cloned = self.audio_buffer.clone();

        if let Some(cache) = &cache_cloned {
            cache::increment_cache(cache);
        }

        Self {
            cache: cache_cloned,
            audio_buffer: buffer_cloned,
            sample_rate: self.sample_rate,
            channels: self.channels,
            pcm_length: self.pcm_length,
            position: 0,
        }
    }
}

impl AudioReader {
    pub fn load_audio_buffer(
        buffer: &[f32],
        sample_rate: f32,
        channels: usize,
        pcm_length: usize,
        owned: bool,
    ) -> Result<Self, AudioReaderError> {
        if buffer.len() == 0 || pcm_length == 0 {
            return Err(AudioReaderError::InvalidPCMLength);
        }

        unsafe {
            let mut config = ma_audio_buffer_config_init(
                ma_format_f32,
                channels as u32,
                pcm_length as u64,
                buffer.as_ptr() as *const c_void,
                std::ptr::null(),
            );

            config.sampleRate = sample_rate as u32;

            let mut audio_buffer = Box::<ma_audio_buffer>::default();
            let result = if owned {
                ma_audio_buffer_init_copy(&config, audio_buffer.as_mut())
            } else {
                ma_audio_buffer_init(&config, audio_buffer.as_mut())
            };

            if result != MA_SUCCESS {
                return Err(AudioReaderError::InitializationError(result));
            }

            Ok(Self {
                cache: None,
                audio_buffer: Some(audio_buffer),
                sample_rate,
                channels: channels as usize,
                pcm_length: pcm_length as usize,
                position: 0,
            })
        }
    }

    pub fn load_cache(cache: Arc<cache::AudioCache>) -> Result<Self, AudioReaderError> {
        cache::increment_cache(&cache);

        let sample_rate = cache.sample_rate;
        let channels = cache.channel_count;
        let pcm_length = cache.length_in_frames;
        let audio_buffer = cache.create_ma_buffer();

        Ok(Self {
            cache: Some(cache),
            audio_buffer: Some(audio_buffer),
            sample_rate,
            channels,
            pcm_length,
            position: 0,
        })
    }

    pub fn read(&mut self, output: &mut [f32]) -> Result<usize, AudioReaderError> {
        let frame_count = output.len() / self.channels as usize;
        if frame_count == 0 {
            return Err(AudioReaderError::InvalidPCMLength);
        }

        let frames_readed;
        let result = unsafe {
            let Some(audio_buffer) = self.audio_buffer.as_mut() else {
                return Err(AudioReaderError::InvalidOperation);
            };
            frames_readed = ma_audio_buffer_read_pcm_frames(
                audio_buffer.as_mut(),
                output.as_mut_ptr() as *mut c_void,
                frame_count as u64,
                0,
            ) as usize;

            MA_SUCCESS
        };

        if result != MA_SUCCESS {
            return Err(AudioReaderError::InvalidOperation);
        }

        self.position += frames_readed;

        Ok(frames_readed)
    }

    pub fn seek(&mut self, position: usize) -> Result<(), AudioReaderError> {
        if position > self.pcm_length {
            return Err(AudioReaderError::SeekError(-1));
        }

        if position == self.position {
            return Ok(());
        }

        let Some(audio_buffer) = self.audio_buffer.as_mut() else {
            return Err(AudioReaderError::InvalidOperation);
        };

        let result =
            unsafe { ma_audio_buffer_seek_to_pcm_frame(audio_buffer.as_mut(), position as u64) };

        if result != MA_SUCCESS {
            return Err(AudioReaderError::SeekError(result));
        }

        self.position = position;
        Ok(())
    }

    pub fn available_frames(&mut self) -> usize {
        self.pcm_length - self.position
    }
}

impl Drop for AudioReader {
    fn drop(&mut self) {
        if let Some(mut audio_buffer) = self.audio_buffer.take() {
            unsafe { ma_audio_buffer_uninit(audio_buffer.as_mut()) };
        }

        let taken_cache = self.cache.take();
        if let Some(cache) = taken_cache {
            cache::return_file_cache(cache);
        }
    }
}

#[derive(Debug, Error)]
pub enum AudioReaderError {
    #[error("Invalid parameter")]
    InvalidParameter,
    #[error("File not found: {0}")]
    FileNotFound(String),
    #[error("Initialization error with code: {}, {}", .0, self.ma_to_string_result())]
    InitializationError(i32),
    #[error("Invalid PCM length")]
    InvalidPCMLength,
    #[error("Invalid operation")]
    InvalidOperation,
    #[error("Seek error with code: {}, {}", .0, self.ma_to_string_result())]
    SeekError(i32),
    #[error("{0}")]
    Other(Box<dyn std::error::Error + Send + 'static>),
}

impl AudioReaderError {
    pub fn from_other<E: std::error::Error + Send + 'static>(error: E) -> Self {
        AudioReaderError::Other(Box::new(error))
    }

    pub fn ma_to_string_result(&self) -> &str {
        match self {
            AudioReaderError::InitializationError(code) | AudioReaderError::SeekError(code) => {
                utils::ma_to_string_result(*code)
            }
            _ => "N/A",
        }
    }
}

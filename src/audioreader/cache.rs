use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::utils;
use miniaudio_sys::*;

use super::{AudioReaderError, ogg};

#[derive(Debug)]
pub struct AudioCache {
    pub buffer: Vec<f32>,
    pub channel_count: usize,
    pub length_in_frames: usize,
    pub sample_rate: f32,
}

impl AudioCache {
    pub fn create_ma_buffer(&self) -> Box<ma_audio_buffer> {
        unsafe {
            let mut config = ma_audio_buffer_config_init(
                ma_format_f32,
                self.channel_count as u32,
                self.length_in_frames as u64,
                &self.buffer[0] as *const f32 as *const std::ffi::c_void,
                std::ptr::null(),
            );

            config.sampleRate = self.sample_rate as u32;

            let buffer: Box<ma_audio_buffer> = Box::new(std::mem::zeroed());
            let result = ma_audio_buffer_init(
                &config,
                buffer.as_ref() as *const ma_audio_buffer as *mut ma_audio_buffer,
            );

            if result != MA_SUCCESS {
                panic!(
                    "Failed to create ma_audio_buffer: {}",
                    utils::ma_to_string_result(result)
                );
            }

            buffer
        }
    }
}

pub(crate) struct Handle {
    pub buffer: Arc<AudioCache>,
    pub lifetime: usize,
}

static AUDIO_READER_CACHE: Lazy<Mutex<HashMap<String, Handle>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn load_file_cache(path: &str) -> Result<Arc<AudioCache>, AudioReaderError> {
    if path.is_empty() {
        return Err(AudioReaderError::InvalidParameter);
    }

    if !std::path::Path::new(path).exists() {
        return Err(AudioReaderError::FileNotFound(path.to_string()));
    }

    let mut cache = AUDIO_READER_CACHE.lock().unwrap();

    if let Some(data) = cache.get_mut(path) {
        data.lifetime += 1;
        return Ok(data.buffer.clone());
    }

    if ogg::is_ogg(path) {
        match ogg::read_ogg_data_file(path) {
            Ok(buffer) => {
                let audio_cache = AudioCache {
                    buffer: buffer.pcm_f32,
                    channel_count: buffer.channels as usize,
                    sample_rate: buffer.sample_rate,
                    length_in_frames: buffer.pcm_length,
                };

                let arc_cache = Arc::new(audio_cache);
                cache.insert(
                    path.to_string(),
                    Handle {
                        buffer: Arc::clone(&arc_cache),
                        lifetime: 1,
                    },
                );

                return Ok(arc_cache);
            }
            Err(e) => {
                return Err(AudioReaderError::from_other(e));
            }
        }
    } else {
        unsafe {
            let cpath = std::ffi::CString::new(path).unwrap();

            let decoder_config = ma_decoder_config_init(ma_format_f32, 0, 0);
            let mut decoder: ma_decoder = std::mem::zeroed();
            let result = ma_decoder_init_file(
                cpath.as_ptr() as *const i8,
                &decoder_config,
                &mut decoder as *mut ma_decoder,
            );

            if result != MA_SUCCESS {
                return Err(AudioReaderError::InitializationError(result));
            }

            let mut pcm_frame = 0;
            let result = ma_decoder_get_length_in_pcm_frames(&mut decoder, &mut pcm_frame);
            if result != MA_SUCCESS {
                ma_decoder_uninit(&mut decoder);
                return Err(AudioReaderError::InitializationError(result));
            }

            let mut pcm_f32: Vec<f32> =
                vec![0.0; (pcm_frame * decoder.outputChannels as u64) as usize];
            let mut frames_read: u64 = 0;
            let result = ma_decoder_read_pcm_frames(
                &mut decoder,
                &mut pcm_f32[0] as *mut f32 as *mut std::ffi::c_void,
                pcm_frame,
                &mut frames_read,
            );

            if result != MA_SUCCESS {
                ma_decoder_uninit(&mut decoder);
                return Err(AudioReaderError::InitializationError(result));
            }

            let buffer = AudioCache {
                buffer: pcm_f32,
                channel_count: decoder.outputChannels as usize,
                sample_rate: decoder.outputSampleRate as f32,
                length_in_frames: pcm_frame as usize,
            };

            ma_decoder_uninit(&mut decoder);

            let arc_cache = Arc::new(buffer);
            cache.insert(
                path.to_string(),
                Handle {
                    buffer: Arc::clone(&arc_cache),
                    lifetime: 1,
                },
            );

            return Ok(arc_cache);
        }
    }
}

pub fn load_buffer_cache(buffer: &[u8]) -> Result<Arc<AudioCache>, AudioReaderError> {
    let key = hash_buffer(buffer);
    let mut cache = AUDIO_READER_CACHE.lock().unwrap();

    if let Some(data) = cache.get_mut(&key) {
        data.lifetime += 1;
        return Ok(data.buffer.clone());
    }

    if ogg::is_ogg_buffer(buffer) {
        match ogg::read_ogg_data_buffer(buffer) {
            Ok(buffer) => {
                let audio_cache = AudioCache {
                    buffer: buffer.pcm_f32,
                    channel_count: buffer.channels as usize,
                    sample_rate: buffer.sample_rate,
                    length_in_frames: buffer.pcm_length as usize,
                };

                let arc_cache = Arc::new(audio_cache);
                cache.insert(
                    key.clone(),
                    Handle {
                        buffer: Arc::clone(&arc_cache),
                        lifetime: 1,
                    },
                );

                return Ok(arc_cache);
            }
            Err(e) => {
                return Err(AudioReaderError::from_other(e));
            }
        }
    } else {
        unsafe {
            let decoder_config = ma_decoder_config_init(ma_format_f32, 0, 0);
            let mut decoder: ma_decoder = std::mem::zeroed();
            let result = ma_decoder_init_memory(
                buffer.as_ptr() as *const std::ffi::c_void,
                buffer.len(),
                &decoder_config,
                &mut decoder as *mut ma_decoder,
            );

            if result != MA_SUCCESS {
                return Err(AudioReaderError::InitializationError(result));
            }

            let mut pcm_frame = 0;
            let result = ma_decoder_get_length_in_pcm_frames(&mut decoder, &mut pcm_frame);
            if result != MA_SUCCESS {
                ma_decoder_uninit(&mut decoder);
                return Err(AudioReaderError::InitializationError(result));
            }

            let mut pcm_f32: Vec<f32> =
                vec![0.0; (pcm_frame * decoder.outputChannels as u64) as usize];
            let mut frames_read: u64 = 0;
            let result = ma_decoder_read_pcm_frames(
                &mut decoder,
                &mut pcm_f32[0] as *mut f32 as *mut std::ffi::c_void,
                pcm_frame,
                &mut frames_read,
            );

            if result != MA_SUCCESS {
                ma_decoder_uninit(&mut decoder);
                return Err(AudioReaderError::InitializationError(result));
            }

            let buffer = AudioCache {
                buffer: pcm_f32,
                channel_count: decoder.outputChannels as usize,
                sample_rate: decoder.outputSampleRate as f32,
                length_in_frames: pcm_frame as usize,
            };

            ma_decoder_uninit(&mut decoder);

            let arc_cache = Arc::new(buffer);
            cache.insert(
                key.clone(),
                Handle {
                    buffer: Arc::clone(&arc_cache),
                    lifetime: 1,
                },
            );

            return Ok(arc_cache);
        }
    }
}

pub fn increment_cache(cache: &Arc<AudioCache>) {
    let mut audio_cache = AUDIO_READER_CACHE.lock().unwrap();

    // Find the buffer and increment its lifetime directly
    if let Some((_key, value)) = audio_cache
        .iter_mut()
        .find(|(_, v)| Arc::ptr_eq(&v.buffer, &cache))
    {
        value.lifetime += 1;
    }
}

pub fn return_file_cache(buf: Arc<AudioCache>) {
    let mut cache = AUDIO_READER_CACHE.lock().unwrap();

    // First, find the key without mutably borrowing the map
    let key = cache
        .iter()
        .find(|(_, v)| Arc::ptr_eq(&v.buffer, &buf))
        .map(|(k, _)| k.clone());

    // Then, mutate or remove the entry
    if let Some(key) = key {
        let remove_entry = {
            let data = cache.get_mut(&key).unwrap();
            if data.lifetime > 0 {
                data.lifetime -= 1;
            }
            data.lifetime == 0
        };

        if remove_entry {
            cache.remove(&key);
        }
    }
}

pub fn hash_buffer(buffer: &[u8]) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(buffer);
    let result = hasher.finalize();
    format!("{:x}", result)
}

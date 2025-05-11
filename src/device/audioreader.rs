use std::{
    io::{BufReader, Cursor, Read, Seek},
    os::raw::c_void,
};

use lewton::inside_ogg::OggStreamReader;
use miniaudio_sys::*;

use crate::utils;

pub struct AudioReader {
    pub decoder: Option<Box<ma_decoder>>,
    pub audio_buffer: Option<Box<ma_audio_buffer>>,

    pub sample_rate: u32,
    pub channels: u32,
    pub pcm_length: u64,
    pub position: u64,
}

impl AudioReader {
    pub fn load(file_path: &str) -> Result<Self, String> {
        if !std::path::Path::new(file_path).exists() {
            return Err(format!("File not found: {}", file_path));
        }

        if is_ogg(file_path) {
            let audio_buffer = read_ogg_data_file(file_path)?;
            let sample_rate = audio_buffer.ref_.sampleRate;
            let channels = audio_buffer.ref_.channels;
            let pcm_length = audio_buffer.ref_.sizeInFrames;

            return Ok(Self {
                decoder: None,
                audio_buffer: Some(audio_buffer),
                sample_rate,
                channels,
                pcm_length,
                position: 0,
            });
        }

        let c_file_path = std::ffi::CString::new(file_path).map_err(|e| e.to_string())?;
        unsafe {
            let mut decoder = Box::<ma_decoder>::new_uninit();
            let decoder_config = ma_decoder_config_init(ma_format_f32, 2, 44100);

            let result = ma_decoder_init_file(
                c_file_path.as_ptr() as *const i8,
                &decoder_config,
                decoder.as_mut_ptr() as *mut ma_decoder,
            );

            if result != MA_SUCCESS {
                return Err(format!(
                    "Failed to initialize decoder: {}",
                    utils::ma_to_string_result(result)
                ));
            }

            let mut decoder = decoder.assume_init();

            let mut pcm_length = 0;
            let result = ma_decoder_get_length_in_pcm_frames(decoder.as_mut(), &mut pcm_length);
            if result != MA_SUCCESS {
                ma_decoder_uninit(decoder.as_mut());

                return Err(format!(
                    "Failed to get PCM length: {}",
                    utils::ma_to_string_result(result)
                ));
            }

            if pcm_length == 0 {
                ma_decoder_uninit(decoder.as_mut());

                return Err("PCM length is zero".to_string());
            }

            let sample_rate = decoder_config.sampleRate;
            let channels = decoder_config.channels;

            Ok(Self {
                decoder: Some(decoder),
                audio_buffer: None,
                sample_rate,
                channels,
                pcm_length,
                position: 0,
            })
        }
    }

    pub fn load_file_buffer(buffer: &[u8]) -> Result<Self, String> {
        if is_ogg_buffer(buffer) {
            let audio_buffer = read_ogg_data_buffer(buffer)?;
            let sample_rate = audio_buffer.ref_.sampleRate;
            let channels = audio_buffer.ref_.channels;
            let pcm_length = audio_buffer.ref_.sizeInFrames;

            return Ok(Self {
                decoder: None,
                audio_buffer: Some(audio_buffer),
                sample_rate,
                channels,
                pcm_length,
                position: 0,
            });
        }

        unsafe {
            let mut decoder = Box::<ma_decoder>::new_uninit();
            let decoder_config = ma_decoder_config_init(ma_format_f32, 2, 44100);

            let result = ma_decoder_init_memory(
                buffer.as_ptr() as *const c_void,
                buffer.len(),
                &decoder_config,
                decoder.as_mut_ptr() as *mut ma_decoder,
            );

            if result != MA_SUCCESS {
                return Err(format!(
                    "Failed to initialize decoder: {}",
                    utils::ma_to_string_result(result)
                ));
            }

            let mut decoder = decoder.assume_init();

            let mut pcm_length = 0;
            let result = ma_decoder_get_length_in_pcm_frames(decoder.as_mut(), &mut pcm_length);
            if result != MA_SUCCESS {
                ma_decoder_uninit(decoder.as_mut());

                return Err(format!(
                    "Failed to get PCM length: {}",
                    utils::ma_to_string_result(result)
                ));
            }

            if pcm_length == 0 {
                ma_decoder_uninit(decoder.as_mut());

                return Err("PCM length is zero".to_string());
            }

            let sample_rate = decoder_config.sampleRate;
            let channels = decoder_config.channels;

            Ok(Self {
                decoder: Some(decoder),
                audio_buffer: None,
                sample_rate,
                channels,
                pcm_length,
                position: 0,
            })
        }
    }

    pub fn load_audio_buffer(
        buffer: &[f32],
        sample_rate: u32,
        channels: u32,
        pcm_length: u64,
        owned: bool,
    ) -> Result<Self, String> {
        unsafe {
            let mut audio_buffer = Box::<ma_audio_buffer>::new_uninit();
            let mut config = ma_audio_buffer_config_init(
                ma_format_f32,
                channels,
                pcm_length,
                buffer.as_ptr() as *const c_void,
                std::ptr::null(),
            );

            config.sampleRate = sample_rate;

            let result = {
                if owned {
                    ma_audio_buffer_init_copy(
                        &config,
                        audio_buffer.as_mut_ptr() as *mut ma_audio_buffer,
                    )
                } else {
                    ma_audio_buffer_init(&config, audio_buffer.as_mut_ptr() as *mut ma_audio_buffer)
                }
            };

            if result != MA_SUCCESS {
                return Err(format!(
                    "Failed to initialize audio buffer: {}",
                    utils::ma_to_string_result(result)
                ));
            }

            let audio_buffer = audio_buffer.assume_init();

            Ok(Self {
                decoder: None,
                audio_buffer: Some(audio_buffer),
                sample_rate,
                channels,
                pcm_length,
                position: 0,
            })
        }
    }

    pub fn read(&mut self, buffer: &mut [f32], size: u64) -> Result<u64, String> {
        if size == 0 {
            return Err("Size must be greater than 0".to_string());
        }

        let expected_array_size = (size * self.channels as u64) as usize;
        if buffer.len() < expected_array_size {
            return Err(format!(
                "Buffer size is too small. Expected: {}, Actual: {}",
                expected_array_size,
                buffer.len()
            ));
        }

        let mut frames_readed = 0;

        let result = unsafe {
            if let Some(audio_buffer) = self.audio_buffer.as_mut() {
                let _frames_readed = ma_audio_buffer_read_pcm_frames(
                    audio_buffer.as_mut(),
                    buffer.as_mut_ptr() as *mut c_void,
                    size,
                    0,
                );

                frames_readed = _frames_readed as u64;

                MA_SUCCESS
            } else if let Some(decoder) = self.decoder.as_mut() {
                ma_decoder_read_pcm_frames(
                    decoder.as_mut(),
                    buffer.as_mut_ptr() as *mut c_void,
                    size,
                    &mut frames_readed,
                )
            } else {
                return Err("Decoder not initialized".to_string());
            }
        };

        if result != MA_SUCCESS {
            return Err(format!(
                "Failed to read PCM frames: {}",
                utils::ma_to_string_result(result)
            ));
        }

        self.position += frames_readed;
        Ok(frames_readed)
    }

    pub fn seek(&mut self, position: u64) -> Result<(), String> {
        if let Some(decoder) = self.decoder.as_mut() {
            let result = unsafe { ma_decoder_seek_to_pcm_frame(decoder.as_mut(), position) };
            if result != MA_SUCCESS {
                return Err(format!(
                    "Failed to seek PCM frame: {}",
                    utils::ma_to_string_result(result)
                ));
            }
        } else if let Some(audio_buffer) = self.audio_buffer.as_mut() {
            let result =
                unsafe { ma_audio_buffer_seek_to_pcm_frame(audio_buffer.as_mut(), position) };
            if result != MA_SUCCESS {
                return Err(format!(
                    "Failed to seek PCM frame: {}",
                    utils::ma_to_string_result(result)
                ));
            }
        } else {
            return Err("Decoder not initialized".to_string());
        }

        self.position = position;
        Ok(())
    }

    pub fn available_frames(&mut self) -> u64 {
        self.pcm_length - self.position
    }
}

impl Drop for AudioReader {
    fn drop(&mut self) {
        if let Some(mut decoder) = self.decoder.take() {
            unsafe { ma_decoder_uninit(decoder.as_mut()) };
        }

        if let Some(mut audio_buffer) = self.audio_buffer.take() {
            unsafe { ma_audio_buffer_uninit(audio_buffer.as_mut()) };
        }
    }
}

pub fn is_ogg(file_path: &str) -> bool {
    const OGG_HEADER: &[u8] = b"OggS";

    if let Ok(mut file) = std::fs::File::open(file_path) {
        let mut buffer = [0; 4];
        if let Ok(_) = file.read_exact(&mut buffer) {
            return &buffer == OGG_HEADER;
        }
    }

    false
}

pub fn is_ogg_buffer(buffer: &[u8]) -> bool {
    const OGG_HEADER: &[u8] = b"OggS";
    if buffer.len() < 4 {
        return false;
    }
    &buffer[0..4] == OGG_HEADER
}

pub fn read_ogg_data_file(file_path: &str) -> Result<Box<ma_audio_buffer>, String> {
    if !is_ogg(file_path) {
        return Err(format!("File is not a valid OGG file: {}", file_path));
    }

    let file = std::fs::File::open(file_path).map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(file);

    let _type = get_ogg_type(&mut reader).map_err(|e| e.to_string())?;

    reader
        .seek(std::io::SeekFrom::Start(0x0))
        .map_err(|e| e.to_string())?;

    match _type {
        Some(OggType::Opus) => {
            return Err("Opus format is not supported yet".to_string());
        }
        Some(OggType::Vorbis) => {
            let reader = OggStreamReader::new(reader).map_err(|e| e.to_string())?;

            return read_ogg_vorbis(reader);
        }
        _ => {
            return Err("Unknown OGG format".to_string());
        }
    }
}

pub fn read_ogg_data_buffer(buffer: &[u8]) -> Result<Box<ma_audio_buffer>, String> {
    if !is_ogg_buffer(buffer) {
        return Err("Buffer is not a valid OGG file".to_string());
    }

    let mut reader = BufReader::new(Cursor::new(buffer));
    let _type = get_ogg_type(&mut reader).map_err(|e| e.to_string())?;

    reader
        .seek(std::io::SeekFrom::Start(0x0))
        .map_err(|e| e.to_string())?;

    match _type {
        Some(OggType::Opus) => {
            return Err("Opus format is not supported yet".to_string());
        }
        Some(OggType::Vorbis) => {
            let reader = OggStreamReader::new(reader).map_err(|e| e.to_string())?;

            return read_ogg_vorbis(reader);
        }
        _ => {
            return Err("Unknown OGG format".to_string());
        }
    }
}

fn read_ogg_vorbis<T: Read + Seek>(
    mut reader: OggStreamReader<T>,
) -> Result<Box<ma_audio_buffer>, String> {
    let mut pcm_f32 = Vec::new();

    while let Ok(Some(packet)) = reader.read_dec_packet_itl() {
        let converted: Vec<f32> = packet.iter().map(|&x| x as f32 / i16::MAX as f32).collect();
        pcm_f32.extend(converted);
    }

    let sample_rate = reader.ident_hdr.audio_sample_rate;
    let channels = reader.ident_hdr.audio_channels as u32;
    let pcm_length = pcm_f32.len() / channels as usize;
    let mut audio_buffer = Box::<ma_audio_buffer>::new_uninit();

    unsafe {
        let mut config = ma_audio_buffer_config_init(
            ma_format_f32,
            channels,
            pcm_length as u64,
            pcm_f32.as_ptr() as *const c_void,
            std::ptr::null(),
        );

        config.sampleRate = sample_rate;

        let result =
            ma_audio_buffer_init_copy(&config, audio_buffer.as_mut_ptr() as *mut ma_audio_buffer);

        if result != MA_SUCCESS {
            return Err(format!(
                "Failed to initialize audio buffer: {}",
                utils::ma_to_string_result(result)
            ));
        }

        let audio_buffer = audio_buffer.assume_init();

        Ok(audio_buffer)
    }
}

// fn read_ogg_opus()

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OggType {
    Unknown,
    Vorbis,
    Opus,
}

pub fn get_ogg_type<T: Read + Seek>(reader: &mut T) -> Result<Option<OggType>, String> {
    // check header
    let mut header = [0; 4];
    if reader.read_exact(&mut header).is_err() {
        return Err("Failed to read OGG header".to_string());
    }

    if &header != b"OggS" {
        return Err("Invalid OGG header".to_string());
    }

    reader
        .seek(std::io::SeekFrom::Start(0x1C))
        .map_err(|e| e.to_string())?;

    let mut data = [0u8; 8];
    if reader.read_exact(&mut data).is_err() {
        return Err("Failed to read OGG data".to_string());
    }

    let mut ogg_type = OggType::Unknown;
    if data.starts_with(b"OpusHead") {
        ogg_type = OggType::Opus;
    } else if data.starts_with(b"\x01vorbis") {
        ogg_type = OggType::Vorbis;
    }

    Ok(Some(ogg_type))
}

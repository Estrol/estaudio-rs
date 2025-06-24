use std::{
    io::{BufReader, Cursor, Read, Seek},
    os::raw::c_void,
};

use lewton::inside_ogg::OggStreamReader;
use miniaudio_sys::*;

use crate::utils;

#[derive(Debug, Clone, PartialEq)]
pub enum AudioReaderError {
    FileNotFound(String),
    OggError(AudioOggError),
    InitializationError(i32),
    InvalidFileFormat,
    InvalidPCMLength,
    InvalidOperation,
    PCMLengthTooLarge,
    BufferTooSmall { expected: usize, actual: usize },
    SeekError(i32),
}

impl std::fmt::Display for AudioReaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioReaderError::FileNotFound(path) => write!(f, "File not found: {}", path),
            AudioReaderError::OggError(e) => write!(f, "OGG error: {:?}", e),
            AudioReaderError::InitializationError(code) => {
                write!(
                    f,
                    "Initialization error with code: {} ({})",
                    code,
                    utils::ma_to_string_result(*code)
                )
            }
            AudioReaderError::InvalidFileFormat => write!(f, "Invalid file format"),
            AudioReaderError::InvalidPCMLength => write!(f, "Invalid PCM length"),
            AudioReaderError::InvalidOperation => write!(f, "Invalid operation"),
            AudioReaderError::PCMLengthTooLarge => write!(f, "PCM length is too large"),
            AudioReaderError::BufferTooSmall { expected, actual } => {
                write!(f, "Buffer too small: expected {}, got {}", expected, actual)
            }
            AudioReaderError::SeekError(code) => write!(
                f,
                "Seek error with code: {} ({})",
                code,
                utils::ma_to_string_result(*code)
            ),
        }
    }
}

pub struct AudioReader {
    pub decoder: Option<Box<ma_decoder>>,
    pub audio_buffer: Option<Box<ma_audio_buffer>>,

    pub sample_rate: u32,
    pub channels: u32,
    pub pcm_length: u64,
    pub position: u64,
}

impl AudioReader {
    pub fn load(file_path: &str) -> Result<Self, AudioReaderError> {
        if !std::path::Path::new(file_path).exists() {
            return Err(AudioReaderError::FileNotFound(file_path.to_string()));
        }

        if is_ogg(file_path) {
            let audio_buffer = read_ogg_data_file(file_path);
            if let Err(e) = audio_buffer {
                return Err(AudioReaderError::OggError(e));
            }

            let audio_buffer = audio_buffer.unwrap();

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

        let c_file_path = std::ffi::CString::new(file_path);
        if let Err(_) = c_file_path {
            return Err(AudioReaderError::InvalidFileFormat);
        }

        let c_file_path = c_file_path.unwrap();

        unsafe {
            let mut decoder = Box::<ma_decoder>::new_uninit();
            let decoder_config = ma_decoder_config_init(ma_format_f32, 2, 44100);

            let result = ma_decoder_init_file(
                c_file_path.as_ptr() as *const i8,
                &decoder_config,
                decoder.as_mut_ptr() as *mut ma_decoder,
            );

            if result != MA_SUCCESS {
                return Err(AudioReaderError::InitializationError(result));
            }

            let mut decoder = decoder.assume_init();

            let mut pcm_length = 0;
            let result = ma_decoder_get_length_in_pcm_frames(decoder.as_mut(), &mut pcm_length);
            if result != MA_SUCCESS {
                ma_decoder_uninit(decoder.as_mut());

                // return Err(format!(
                //     "Failed to get PCM length: {}",
                //     utils::ma_to_string_result(result)
                // ));
                return Err(AudioReaderError::InitializationError(result));
            }

            if pcm_length == 0 {
                ma_decoder_uninit(decoder.as_mut());

                return Err(AudioReaderError::InvalidPCMLength);
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

    pub fn load_file_buffer(buffer: &[u8]) -> Result<Self, AudioReaderError> {
        if is_ogg_buffer(buffer) {
            let audio_buffer = read_ogg_data_buffer(buffer);
            if let Err(e) = audio_buffer {
                return Err(AudioReaderError::OggError(e));
            }

            let audio_buffer = audio_buffer.unwrap();

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
                return Err(AudioReaderError::InitializationError(result));
            }

            let mut decoder = decoder.assume_init();

            let mut pcm_length = 0;
            let result = ma_decoder_get_length_in_pcm_frames(decoder.as_mut(), &mut pcm_length);
            if result != MA_SUCCESS {
                ma_decoder_uninit(decoder.as_mut());

                return Err(AudioReaderError::InitializationError(result));
            }

            if pcm_length == 0 {
                ma_decoder_uninit(decoder.as_mut());

                return Err(AudioReaderError::InvalidPCMLength);
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
    ) -> Result<Self, AudioReaderError> {
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
                return Err(AudioReaderError::InitializationError(result));
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

    pub fn read(&mut self, buffer: &mut [f32], size: u64) -> Result<u64, AudioReaderError> {
        if size == 0 {
            return Err(AudioReaderError::InvalidPCMLength);
        }

        let expected_array_size = (size * self.channels as u64) as usize;
        if buffer.len() < expected_array_size {
            return Err(AudioReaderError::BufferTooSmall {
                expected: expected_array_size,
                actual: buffer.len(),
            });
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
                unreachable!() // Decoder or audio buffer must be initialized
            }
        };

        if result != MA_SUCCESS {
            return Err(AudioReaderError::InvalidOperation);
        }

        self.position += frames_readed;
        Ok(frames_readed)
    }

    pub fn seek(&mut self, position: u64) -> Result<(), AudioReaderError> {
        if let Some(decoder) = self.decoder.as_mut() {
            let result = unsafe { ma_decoder_seek_to_pcm_frame(decoder.as_mut(), position) };
            if result != MA_SUCCESS {
                return Err(AudioReaderError::SeekError(result));
            }
        } else if let Some(audio_buffer) = self.audio_buffer.as_mut() {
            let result =
                unsafe { ma_audio_buffer_seek_to_pcm_frame(audio_buffer.as_mut(), position) };
            if result != MA_SUCCESS {
                return Err(AudioReaderError::SeekError(result));
            }
        } else {
            unreachable!(); // Decoder or audio buffer must be initialized
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

#[derive(Debug, Clone, PartialEq)]
pub enum AudioOggError {
    InvalidFileFormat,
    UnknownFormat,
    ReadError(&'static str),
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

pub fn read_ogg_data_file(file_path: &str) -> Result<Box<ma_audio_buffer>, AudioOggError> {
    if !is_ogg(file_path) {
        return Err(AudioOggError::InvalidFileFormat);
    }

    let file = std::fs::File::open(file_path);
    if let Err(_) = file {
        return Err(AudioOggError::ReadError("Failed to open OGG file"));
    }

    let file = file.unwrap();

    let mut reader = BufReader::new(file);

    let _type = get_ogg_type(&mut reader);
    if let Err(e) = _type {
        return Err(e);
    }

    let _type = _type.unwrap();

    let err = reader.seek(std::io::SeekFrom::Start(0x0));

    if err.is_err() {
        return Err(AudioOggError::ReadError("Failed to seek in OGG file"));
    }

    match _type {
        Some(OggType::Opus) => {
            return read_ogg_opus(reader);
        }
        Some(OggType::Vorbis) => {
            let reader = OggStreamReader::new(reader);

            if let Err(_) = reader {
                return Err(AudioOggError::ReadError("Failed to read OGG Vorbis data"));
            }

            return read_ogg_vorbis(reader.unwrap());
        }
        _ => {
            return Err(AudioOggError::UnknownFormat);
        }
    }
}

pub fn read_ogg_data_buffer(buffer: &[u8]) -> Result<Box<ma_audio_buffer>, AudioOggError> {
    if !is_ogg_buffer(buffer) {
        return Err(AudioOggError::InvalidFileFormat);
    }

    let mut reader = BufReader::new(Cursor::new(buffer));
    let _type = get_ogg_type(&mut reader);
    if let Err(e) = _type {
        return Err(e);
    }

    let _type = _type.unwrap();

    let err = reader.seek(std::io::SeekFrom::Start(0x0));

    if err.is_err() {
        return Err(AudioOggError::ReadError("Failed to seek in OGG file"));
    }

    match _type {
        Some(OggType::Opus) => {
            return read_ogg_opus(reader);
        }
        Some(OggType::Vorbis) => {
            let reader = OggStreamReader::new(reader);
            if let Err(_) = reader {
                return Err(AudioOggError::ReadError("Failed to read OGG Vorbis data"));
            }

            return read_ogg_vorbis(reader.unwrap());
        }
        _ => {
            return Err(AudioOggError::UnknownFormat);
        }
    }
}

fn read_ogg_vorbis<T: Read + Seek>(
    mut reader: OggStreamReader<T>,
) -> Result<Box<ma_audio_buffer>, AudioOggError> {
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
            return Err(AudioOggError::ReadError(utils::ma_to_string_result(result)));
        }

        let audio_buffer = audio_buffer.assume_init();

        Ok(audio_buffer)
    }
}

fn read_ogg_opus<T: Seek + Read>(data: T) -> Result<Box<ma_audio_buffer>, AudioOggError> {
    let decoded = ogg_opus::decode::<T, 48000>(data);
    if let Err(_) = decoded {
        return Err(AudioOggError::ReadError("Failed to decode OGG Opus data"));
    }

    let decoded = decoded.unwrap();

    let mut pcm_f32 = Vec::new();
    for frame in decoded.0.iter() {
        pcm_f32.push(*frame as f32 / i16::MAX as f32);
    }

    let channel = decoded.1.channels;
    let sample_rate = 48000;

    let pcm_length = pcm_f32.len() / channel as usize;
    let mut audio_buffer = Box::<ma_audio_buffer>::new_uninit();

    unsafe {
        let mut config = ma_audio_buffer_config_init(
            ma_format_f32,
            channel as u32,
            pcm_length as u64,
            pcm_f32.as_ptr() as *const c_void,
            std::ptr::null(),
        );

        config.sampleRate = sample_rate;

        let result =
            ma_audio_buffer_init_copy(&config, audio_buffer.as_mut_ptr() as *mut ma_audio_buffer);

        if result != MA_SUCCESS {
            return Err(AudioOggError::ReadError(utils::ma_to_string_result(result)));
        }

        let audio_buffer = audio_buffer.assume_init();

        Ok(audio_buffer)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OggType {
    Unknown,
    Vorbis,
    Opus,
}

pub fn get_ogg_type<T: Read + Seek>(reader: &mut T) -> Result<Option<OggType>, AudioOggError> {
    // check header
    let mut header = [0; 4];
    if reader.read_exact(&mut header).is_err() {
        return Err(AudioOggError::ReadError("Failed to read OGG header"));
    }

    if &header != b"OggS" {
        return Err(AudioOggError::InvalidFileFormat);
    }

    let err = reader.seek(std::io::SeekFrom::Start(0x1C));

    if err.is_err() {
        return Err(AudioOggError::ReadError("Failed to seek in OGG file"));
    }

    let mut data = [0u8; 8];
    if reader.read_exact(&mut data).is_err() {
        return Err(AudioOggError::ReadError("Failed to read OGG type data"));
    }

    let mut ogg_type = OggType::Unknown;
    if data.starts_with(b"OpusHead") {
        ogg_type = OggType::Opus;
    } else if data.starts_with(b"\x01vorbis") {
        ogg_type = OggType::Vorbis;
    }

    Ok(Some(ogg_type))
}

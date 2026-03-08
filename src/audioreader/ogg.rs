use std::io::{BufReader, Cursor, Read, Seek};

use lewton::inside_ogg::OggStreamReader;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum OggError {
    #[error("Invalid file format")]
    InvalidFileFormat,
    #[error("Unknown OGG format")]
    UnknownFormat,
    #[error("Read error: {0}")]
    ReadError(&'static str),
}

const OGG_HEADER: &[u8] = b"OggS";

pub fn is_ogg(file_path: &str) -> bool {
    if let Ok(mut file) = std::fs::File::open(file_path) {
        let mut buffer = [0; 4];
        if let Ok(_) = file.read_exact(&mut buffer) {
            return &buffer == OGG_HEADER;
        }
    }

    false
}

pub fn is_ogg_buffer(buffer: &[u8]) -> bool {
    if buffer.len() < 4 {
        return false;
    }
    &buffer[0..4] == OGG_HEADER
}

pub fn read_ogg_data_file(file_path: &str) -> Result<OggBuffer, OggError> {
    if !is_ogg(file_path) {
        return Err(OggError::InvalidFileFormat);
    }

    let file = std::fs::File::open(file_path);
    if let Err(_) = file {
        return Err(OggError::ReadError("Failed to open OGG file"));
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
        return Err(OggError::ReadError("Failed to seek in OGG file"));
    }

    match _type {
        Some(OggType::Opus) => {
            return read_ogg_opus(reader);
        }
        Some(OggType::Vorbis) => {
            let reader = OggStreamReader::new(reader);

            if let Err(_) = reader {
                return Err(OggError::ReadError("Failed to read OGG Vorbis data"));
            }

            return read_ogg_vorbis(reader.unwrap());
        }
        _ => {
            return Err(OggError::UnknownFormat);
        }
    }
}

pub fn read_ogg_data_buffer(buffer: &[u8]) -> Result<OggBuffer, OggError> {
    if !is_ogg_buffer(buffer) {
        return Err(OggError::InvalidFileFormat);
    }

    let mut reader = BufReader::new(Cursor::new(buffer));
    let _type = get_ogg_type(&mut reader);
    if let Err(e) = _type {
        return Err(e);
    }

    let _type = _type.unwrap();

    let err = reader.seek(std::io::SeekFrom::Start(0x0));

    if err.is_err() {
        return Err(OggError::ReadError("Failed to seek in OGG file"));
    }

    match _type {
        Some(OggType::Opus) => {
            return read_ogg_opus(reader);
        }
        Some(OggType::Vorbis) => {
            let reader = OggStreamReader::new(reader);
            if let Err(_) = reader {
                return Err(OggError::ReadError("Failed to read OGG Vorbis data"));
            }

            return read_ogg_vorbis(reader.unwrap());
        }
        _ => {
            return Err(OggError::UnknownFormat);
        }
    }
}

pub struct OggBuffer {
    pub pcm_f32: Vec<f32>,
    pub sample_rate: f32,
    pub channels: u32,
    pub pcm_length: usize,
}

fn read_ogg_vorbis<T: Read + Seek>(mut reader: OggStreamReader<T>) -> Result<OggBuffer, OggError> {
    let mut pcm_f32 = Vec::new();

    while let Ok(Some(packet)) = reader.read_dec_packet_itl() {
        let converted: Vec<f32> = packet.iter().map(|&x| x as f32 / i16::MAX as f32).collect();
        pcm_f32.extend(converted);
    }

    let sample_rate = reader.ident_hdr.audio_sample_rate as f32;
    let channels = reader.ident_hdr.audio_channels as u32;
    let pcm_length = pcm_f32.len() / channels as usize;

    return Ok(OggBuffer {
        pcm_f32,
        sample_rate,
        channels,
        pcm_length,
    });
}

fn read_ogg_opus<T: Seek + Read>(data: T) -> Result<OggBuffer, OggError> {
    let decoded = ogg_opus::decode::<T, 48000>(data);
    if let Err(_) = decoded {
        return Err(OggError::ReadError("Failed to decode OGG Opus data"));
    }

    let decoded = decoded.unwrap();

    let mut pcm_f32 = Vec::new();
    for frame in decoded.0.iter() {
        pcm_f32.push(*frame as f32 / i16::MAX as f32);
    }

    const SAMPLE_RATE_OPUS: f32 = 48000.0;
    let channel = decoded.1.channels;
    let pcm_length = pcm_f32.len() / channel as usize;

    return Ok(OggBuffer {
        pcm_f32,
        sample_rate: SAMPLE_RATE_OPUS,
        channels: channel as u32,
        pcm_length,
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OggType {
    Unknown,
    Vorbis,
    Opus,
}

pub fn get_ogg_type<T: Read + Seek>(reader: &mut T) -> Result<Option<OggType>, OggError> {
    // check header
    let mut header = [0; 4];
    if reader.read_exact(&mut header).is_err() {
        return Err(OggError::ReadError("Failed to read OGG header"));
    }

    if &header != b"OggS" {
        return Err(OggError::InvalidFileFormat);
    }

    let err = reader.seek(std::io::SeekFrom::Start(0x1C));

    if err.is_err() {
        return Err(OggError::ReadError("Failed to seek in OGG file"));
    }

    let mut data = [0u8; 8];
    if reader.read_exact(&mut data).is_err() {
        return Err(OggError::ReadError("Failed to read OGG type data"));
    }

    let mut ogg_type = OggType::Unknown;
    if data.starts_with(b"OpusHead") {
        ogg_type = OggType::Opus;
    } else if data.starts_with(b"\x01vorbis") {
        ogg_type = OggType::Vorbis;
    }

    Ok(Some(ogg_type))
}

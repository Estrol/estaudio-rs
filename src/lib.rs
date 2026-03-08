pub(crate) mod effects;
pub(crate) mod math;
pub(crate) mod utils;

pub(crate) mod audioreader;
pub(crate) mod context;
pub(crate) mod device;
pub(crate) mod encoder;
pub(crate) mod macros;
pub(crate) mod misc;
pub(crate) mod mixer;
pub(crate) mod sample;
pub(crate) mod track;

use std::sync::Arc;
use crate::audioreader::cache::AudioCache;

pub use crate::context::{Backend, ContextError, DeviceType, HardwareInfos};

pub use crate::device::{Device, DeviceError, DeviceInfo};

pub use crate::encoder::{Encoder, EncoderError, EncoderInfo, writer::WriteFormat};

pub use crate::mixer::{Mixer, MixerError, MixerInfo, MixerInput};

pub use crate::sample::{Sample, SampleError, SampleInfo};

pub use crate::track::{Track, TrackError, TrackInfo};

pub use crate::misc::{
    audioattributes::AudioAttributes,
    audiopropertyhandler::{PropertyError, PropertyHandler},
};

#[derive(Debug)]
pub struct BufferInfo<'a> {
    pub data: &'a [f32],
    pub channels: usize,
    pub sample_rate: f32,
}

impl BufferInfo<'_> {
    pub fn into_owned(self) -> BufferInfoOwned {
        BufferInfoOwned {
            data: self.data.to_vec(),
            channels: self.channels,
            sample_rate: self.sample_rate,
        }
    }
}

#[derive(Clone, Debug)]
pub struct BufferInfoOwned {
    pub data: Vec<f32>,
    pub channels: usize,
    pub sample_rate: f32,
}

impl BufferInfoOwned {
    pub fn get_ref(&self) -> BufferInfo<'_> {
        BufferInfo {
            data: &self.data,
            channels: self.channels,
            sample_rate: self.sample_rate,
        }
    }
}

#[derive(Default)]
pub enum Source<'a> {
    #[default]
    None,
    Path(&'a str),
    Memory(&'a [u8]),
    Stream(Box<dyn std::io::Read + Send>),
    Buffer(BufferInfo<'a>),
}

impl std::fmt::Debug for Source<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Source::None => write!(f, "Source::None"),
            Source::Path(path) => write!(f, "Source::Path({})", path),
            Source::Memory(_) => write!(f, "Source::Memory(...)"),
            Source::Stream(_) => write!(f, "Source::Stream(...)"),
            Source::Buffer(buffer) => write!(
                f,
                "Source::Buffer {{ data: [...], channels: {}, sample_rate: {} }}",
                buffer.channels, buffer.sample_rate
            ),
        }
    }
}

impl<'a> Source<'a> {
    pub(crate) fn into_buffer(self) -> (Option<Arc<AudioCache>>, Option<BufferInfo<'a>>) {
        use audioreader::cache;

        match self {
            Source::Buffer(buffer_info) => (None, Some(buffer_info)),
            Source::Memory(data) => {
                let Ok(cache) = cache::load_buffer_cache(data) else {
                    eprintln!("Failed to load buffer cache");
                    return (None, None);
                };

                (Some(cache), None)
            }
            Source::Path(path) => {
                let Ok(cache) = cache::load_file_cache(path) else {
                    eprintln!("Failed to load file cache for path: {}", path);
                    return (None, None);
                };

                (Some(cache), None)
            }
            Source::Stream(mut stream) => {
                let mut buf = Vec::new();
                if let Err(e) = stream.read_to_end(&mut buf) {
                    eprintln!("Failed to read from stream: {}", e);
                    return (None, None);
                }

                let Ok(cache) = cache::load_buffer_cache(buf.as_slice()) else {
                    eprintln!("Failed to load buffer cache from stream");
                    return (None, None);
                };

                (Some(cache), None)
            }
            Source::None => (None, None),
        }
    }
}

pub fn enumerate_devices(backends: &[Backend]) -> Result<HardwareInfos, ContextError> {
    context::enumerable(backends)
}

pub fn create_device(
    config: DeviceInfo,
) -> Result<Device, DeviceError> {
    Device::new(config)
}

pub fn create_sample(config: SampleInfo) -> Result<Sample, SampleError> {
    Sample::new(config)
}

pub fn create_track(config: TrackInfo) -> Result<Track, TrackError> {
    Track::new(config)
}

pub fn create_encoder(config: EncoderInfo) -> Result<Encoder, EncoderError> {
    Encoder::new(config)
}

pub fn create_mixer(config: MixerInfo) -> Result<Mixer, MixerError> {
    Mixer::new(config)
}

#[cfg(feature = "capi")]
pub mod capi;

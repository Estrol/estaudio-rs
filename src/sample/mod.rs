use std::sync::{Arc, Mutex, atomic::Ordering};

pub(crate) mod sampelchannel;
pub(crate) mod sampleinner;

use crate::{
    BufferInfoOwned,
    audioreader::cache::AudioCache,
    device::Device,
    effects::AudioFXError,
    misc::{
        audioattributes::AudioAttributes,
        audiopropertyhandler::{PropertyError, PropertyHandler},
    },
    sample::sampleinner::SampleChannelStatus,
};

pub use sampelchannel::SampleChannel;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct SampleAttributes {
    pub enable_fx: bool,
    pub enable_spatialization: bool,

    pub volume: f32,
    pub sample_rate: f32,
    pub pan: f32,

    pub fx_tempo: f32,
    pub fx_pitch: f32,
}

impl Default for SampleAttributes {
    fn default() -> Self {
        Self {
            enable_fx: false,
            enable_spatialization: false,
            volume: 1.0,
            sample_rate: 44100.0,
            pan: 0.0,
            fx_tempo: 1.0,
            fx_pitch: 1.0,
        }
    }
}

#[derive(Default)]
pub struct SampleInfo<'a> {
    pub source: crate::Source<'a>,
    pub sample_rate: Option<f32>,
    pub channels: Option<usize>,
}

#[derive(Default, Clone)]
pub struct SampleChannelInfo {
    pub sample_rate: Option<f32>,
    pub channels: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct Sample {
    pub(crate) cache: Option<Arc<AudioCache>>,
    pub(crate) buffer: Option<BufferInfoOwned>,
    #[allow(dead_code)]
    pub(crate) pcm_length: usize,
    pub(crate) sample_rate: f32,
    pub(crate) channels: usize,
    pub(crate) attributes: Arc<Mutex<SampleAttributes>>,
    pub(crate) handles: Vec<SampleChannel>,
}

impl Sample {
    pub(crate) fn new(info: SampleInfo) -> Result<Self, SampleError> {
        let (cache, buffer_info) = info.source.into_buffer();

        let (cache, buffer, pcm_length, sample_rate, channels) = match buffer_info {
            Some(buffer_info) => {
                let channels = buffer_info.channels;
                let sample_rate = buffer_info.sample_rate;
                let pcm_length = buffer_info.data.len() / channels;

                (
                    None::<Arc<AudioCache>>,
                    Some(buffer_info.into_owned()),
                    pcm_length,
                    sample_rate,
                    channels,
                )
            }
            None => {
                let Some(cache) = cache else {
                    return Err(SampleError::InvalidOperation(
                        "No valid audio source provided",
                    ));
                };

                let sample_rate = cache.sample_rate;
                let channels = cache.channel_count;
                let pcm_length = cache.length_in_frames;

                (Some(cache.clone()), None, pcm_length, sample_rate, channels)
            }
        };

        let attributes = Arc::new(Mutex::new(SampleAttributes {
            sample_rate,
            ..Default::default()
        }));

        let handles = vec![];

        Ok(Self {
            cache,
            buffer,
            pcm_length,
            sample_rate,
            channels,
            handles,
            attributes,
        })
    }

    pub fn get_channel(
        &mut self,
        info: Option<SampleChannelInfo>,
    ) -> Result<SampleChannel, SampleError> {
        let Ok(channel) = self.get_channels(1, info) else {
            return Err(SampleError::NoAvailableChannels);
        };

        Ok(channel.into_iter().next().unwrap())
    }

    pub fn get_channels(
        &mut self,
        size: usize,
        info: Option<SampleChannelInfo>,
    ) -> Result<Vec<SampleChannel>, SampleError> {
        let mut channels = vec![];

        for _ in 0..size {
            let mut channel = self.get_unused_channel();

            if channel.is_none() {
                let handle = SampleChannel::new(
                    &self.cache,
                    &self.buffer.as_ref().map(|e| e.get_ref()),
                    self.channels,
                    self.sample_rate,
                ).map_err(SampleError::from_other)?;

                self.handles.push(handle.clone());
                channel = Some(handle);
            }

            if let Some(mut ch) = channel {
                ch.reset(&info);

                channels.push(ch);
            }
        }

        if channels.is_empty() {
            return Err(SampleError::NoAvailableChannels);
        }

        Ok(channels)
    }

    pub fn play(&mut self, device: &mut Device) -> Result<SampleChannel, SampleError> {
        self.play_ex(device, None)
    }

    pub fn play_ex(
        &mut self,
        device: &mut Device,
        info: Option<SampleChannelInfo>,
    ) -> Result<SampleChannel, SampleError> {
        let Ok(mut channel) = self.get_channel(info) else {
            return Err(SampleError::NoAvailableChannels);
        };

        self.apply_attributes(&mut channel)
            .map_err(SampleError::from_other)?;
        channel.play(device).map_err(SampleError::from_other)?;

        Ok(channel)
    }

    fn get_unused_channel(&mut self) -> Option<SampleChannel> {
        for channel in &self.handles {
            if channel.get_inner_counter() == 1 && channel.is_finished() {
                let mut handle = channel.inner.lock().unwrap();
                handle.seek(0).unwrap();

                handle
                    .status
                    .store(SampleChannelStatus::NotStarted, Ordering::Relaxed);
                return Some(channel.clone());
            }
        }

        None
    }

    fn apply_attributes(&self, channel: &mut SampleChannel) -> Result<(), PropertyError> {
        let attributes = self.attributes.lock().unwrap();

        channel.set_attribute_f32(AudioAttributes::Volume, attributes.volume)?;
        channel.set_attribute_f32(AudioAttributes::Pan, attributes.pan)?;
        channel.set_attribute_f32(AudioAttributes::SampleRate, attributes.sample_rate)?;

        channel.set_attribute_bool(AudioAttributes::FXEnabled, attributes.enable_fx)?;
        channel.set_attribute_bool(
            AudioAttributes::SpatializationEnabled,
            attributes.enable_spatialization,
        )?;

        if attributes.enable_fx {
            channel.set_attribute_f32(AudioAttributes::FXPitch, attributes.fx_pitch)?;
            channel.set_attribute_f32(AudioAttributes::FXTempo, attributes.fx_tempo)?;
        }

        Ok(())
    }
}

impl PropertyHandler for Sample {
    fn get_attribute_f32(&self, _type: AudioAttributes) -> Result<f32, PropertyError> {
        let attributes = self.attributes.lock().unwrap();

        match _type {
            AudioAttributes::SampleRate => Ok(attributes.sample_rate),
            AudioAttributes::Volume => Ok(attributes.volume),
            AudioAttributes::Pan => Ok(attributes.pan),
            AudioAttributes::FXPitch => {
                if !attributes.enable_fx {
                    return Err(PropertyError::from_other(AudioFXError::NotEnabled));
                }

                Ok(attributes.fx_pitch)
            }
            AudioAttributes::FXTempo => {
                if !attributes.enable_fx {
                    return Err(PropertyError::from_other(AudioFXError::NotEnabled));
                }

                Ok(attributes.fx_tempo)
            }
            _ => Err(PropertyError::UnsupportedAttribute("Unknown attribute")),
        }
    }

    fn set_attribute_f32(
        &mut self,
        _type: AudioAttributes,
        _value: f32,
    ) -> Result<(), PropertyError> {
        let mut attributes = self.attributes.lock().unwrap();

        match _type {
            AudioAttributes::SampleRate => {
                attributes.sample_rate = _value;
                Ok(())
            }
            AudioAttributes::Volume => {
                attributes.volume = _value;
                Ok(())
            }
            AudioAttributes::Pan => {
                attributes.pan = _value;
                Ok(())
            }
            AudioAttributes::FXPitch => {
                if !attributes.enable_fx {
                    return Err(PropertyError::from_other(AudioFXError::NotEnabled));
                }

                attributes.fx_pitch = _value;
                Ok(())
            }
            AudioAttributes::FXTempo => {
                if !attributes.enable_fx {
                    return Err(PropertyError::from_other(AudioFXError::NotEnabled));
                }

                attributes.fx_tempo = _value;
                Ok(())
            }
            _ => Err(PropertyError::UnsupportedAttribute("Unknown attribute")),
        }
    }

    fn get_attribute_bool(&self, _type: AudioAttributes) -> Result<bool, PropertyError> {
        let attributes = self.attributes.lock().unwrap();

        match _type {
            AudioAttributes::FXEnabled => Ok(attributes.enable_fx),
            AudioAttributes::SpatializationEnabled => Ok(attributes.enable_spatialization),
            _ => Err(PropertyError::UnsupportedAttribute("Unknown attribute")),
        }
    }

    fn set_attribute_bool(
        &mut self,
        _type: AudioAttributes,
        _value: bool,
    ) -> Result<(), PropertyError> {
        let mut attributes = self.attributes.lock().unwrap();

        match _type {
            AudioAttributes::FXEnabled => {
                attributes.enable_fx = _value;
                Ok(())
            }
            AudioAttributes::SpatializationEnabled => {
                attributes.enable_spatialization = _value;
                Ok(())
            }
            _ => Err(PropertyError::UnsupportedAttribute("Unknown attribute")),
        }
    }
}

#[derive(Debug, Error)]
pub enum SampleError {
    #[error("{0}")]
    InvalidOperation(&'static str),
    #[error("File not found: {0}")]
    FileNotFound(String),
    #[error("Invalid channel count: {0}")]
    InvalidChannels(u32),
    #[error("Invalid sample rate: {0}")]
    InvalidSampleRate(u32),
    #[error("Sample already in use by another device with ref id: {0}")]
    InvalidDeviceRefId(u32),
    #[error("Seek operation failed")]
    SeekFailed,
    #[error("No available channels to play the sample")]
    NoAvailableChannels,
    #[error("Failed to lock Sample")]
    LockFailed,
    #[error("{0}")]
    Other(Box<dyn std::error::Error>),
}

impl SampleError {
    pub fn from_other<E: std::error::Error + 'static>(error: E) -> Self {
        SampleError::Other(Box::new(error))
    }
}

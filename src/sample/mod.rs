use std::sync::{Arc, Mutex};

use crate::{
    channel::{AudioChannel, AudioChannelError},
    device::{
        AudioAttributes, AudioDevice, AudioPropertyError, AudioPropertyHandler,
        audioreader::{AudioReader, AudioReaderError},
    },
    effects::{AudioFXError, AudioPannerError},
};

#[derive(Debug, Clone)]
pub enum AudioSampleError {
    FileNotFound(String),
    InvalidChannels(u32),
    InvalidSampleRate(u32),
    AudioReaderError(AudioReaderError),
    AudioPannerError(AudioPannerError),
    AudioChannelError(AudioChannelError),
    AudioPropertyError(AudioPropertyError),
}

impl std::fmt::Display for AudioSampleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioSampleError::FileNotFound(file) => write!(f, "Audio file not found: {}", file),
            AudioSampleError::InvalidChannels(channels) => {
                write!(f, "Invalid number of channels: {}", channels)
            }
            AudioSampleError::InvalidSampleRate(rate) => write!(f, "Invalid sample rate: {}", rate),
            AudioSampleError::AudioReaderError(e) => write!(f, "Audio reader error: {}", e),
            AudioSampleError::AudioPannerError(e) => write!(f, "Audio panner error: {}", e),
            AudioSampleError::AudioChannelError(e) => write!(f, "Audio channel error: {}", e),
            AudioSampleError::AudioPropertyError(e) => write!(f, "Audio property error: {}", e),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioSampleAttributes {
    pub enable_fx: bool,
    pub enable_spatialization: bool,

    pub volume: f32,
    pub sample_rate: f32,
    pub pan: f32,

    pub fx_tempo: f32,
    pub fx_pitch: f32,
}

impl Default for AudioSampleAttributes {
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

#[derive(Debug, Clone)]
pub struct AudioSample {
    pub(crate) buffer: Vec<f32>,
    pub(crate) pcm_length: u64,
    pub(crate) sample_rate: u32,
    pub(crate) channels: u32,
    pub(crate) attributes: Arc<Mutex<AudioSampleAttributes>>,
}

impl AudioSample {
    pub(crate) fn load(file_path: &str) -> Result<Self, AudioSampleError> {
        let mut audioreader =
            AudioReader::load(file_path).map_err(AudioSampleError::AudioReaderError)?;

        let mut buffer = vec![0.0; audioreader.pcm_length as usize * audioreader.channels as usize];
        audioreader
            .read(&mut buffer, audioreader.pcm_length)
            .map_err(AudioSampleError::AudioReaderError)?;

        let mut attributes = AudioSampleAttributes::default();
        attributes.sample_rate = audioreader.sample_rate as f32;

        Ok(Self {
            buffer,
            pcm_length: audioreader.pcm_length,
            sample_rate: audioreader.sample_rate,
            channels: audioreader.channels,
            attributes: Arc::new(Mutex::new(attributes)),
        })
    }

    pub(crate) fn load_file_buffer(buffer: &[u8]) -> Result<Self, AudioSampleError> {
        let mut audioreader =
            AudioReader::load_file_buffer(buffer).map_err(AudioSampleError::AudioReaderError)?;

        let mut audio_buffer =
            vec![0.0; audioreader.pcm_length as usize * audioreader.channels as usize];
        audioreader
            .read(&mut audio_buffer, audioreader.pcm_length)
            .map_err(AudioSampleError::AudioReaderError)?;

        let mut attributes = AudioSampleAttributes::default();
        attributes.sample_rate = audioreader.sample_rate as f32;

        Ok(Self {
            buffer: audio_buffer,
            pcm_length: audioreader.pcm_length,
            sample_rate: audioreader.sample_rate,
            channels: audioreader.channels,
            attributes: Arc::new(Mutex::new(attributes)),
        })
    }

    pub(crate) fn load_audio_buffer(
        buffer: &[f32],
        pcm_length: u64,
        sample_rate: u32,
        channels: u32,
    ) -> Result<Self, AudioSampleError> {
        if channels < 1 || channels > 8 {
            return Err(AudioSampleError::InvalidChannels(channels));
        }

        if sample_rate < 8000 || sample_rate > 192000 {
            return Err(AudioSampleError::InvalidSampleRate(sample_rate));
        }

        let mut attributes = AudioSampleAttributes::default();
        attributes.sample_rate = sample_rate as f32;

        Ok(Self {
            buffer: buffer.to_vec(),
            pcm_length,
            sample_rate,
            channels,
            attributes: Arc::new(Mutex::new(attributes)),
        })
    }

    pub fn play(&self, device: &AudioDevice) -> Result<(), AudioSampleError> {
        let mut channel = AudioChannel::new_audio_buffer(
            &self.buffer,
            self.pcm_length,
            self.sample_rate,
            self.channels,
        )
        .map_err(AudioSampleError::AudioChannelError)?;

        channel
            .attach(device)
            .map_err(AudioSampleError::AudioChannelError)?;

        self.apply_attributes(&channel)
            .map_err(AudioSampleError::AudioPropertyError)?;

        channel
            .play()
            .map_err(AudioSampleError::AudioChannelError)?;

        Ok(())
    }

    pub fn get_channels(
        &self,
        device: &AudioDevice,
        size: u32,
    ) -> Result<Vec<AudioChannel>, AudioSampleError> {
        let mut channels = vec![];

        for _ in 0..size {
            let mut channel = AudioChannel::new_audio_buffer(
                &self.buffer,
                self.pcm_length,
                self.sample_rate,
                self.channels,
            )
            .map_err(|e| AudioSampleError::AudioChannelError(e))?;

            channel
                .attach(&device)
                .map_err(|e| AudioSampleError::AudioChannelError(e))?;

            self.apply_attributes(&channel)
                .map_err(|e| AudioSampleError::AudioPropertyError(e))?;

            channels.push(channel);
        }

        Ok(channels)
    }

    fn apply_attributes(&self, channel: &AudioChannel) -> Result<(), AudioPropertyError> {
        let attributes = self.attributes.lock().unwrap();

        channel.set_attribute_f32(AudioAttributes::Volume, attributes.volume)?;
        channel.set_attribute_f32(AudioAttributes::Pan, attributes.pan)?;
        channel.set_attribute_f32(AudioAttributes::SampleRate, attributes.sample_rate)?;

        channel.set_attribute_bool(AudioAttributes::AudioFX, attributes.enable_fx)?;
        channel.set_attribute_bool(
            AudioAttributes::AudioSpatialization,
            attributes.enable_spatialization,
        )?;

        if attributes.enable_fx {
            channel.set_attribute_f32(AudioAttributes::FXPitch, attributes.fx_pitch)?;
            channel.set_attribute_f32(AudioAttributes::FXTempo, attributes.fx_tempo)?;
        }

        Ok(())
    }
}

impl AudioPropertyHandler for AudioSample {
    fn get_attribute_f32(&self, _type: AudioAttributes) -> Result<f32, AudioPropertyError> {
        let attributes = self.attributes.lock().unwrap();

        match _type {
            AudioAttributes::SampleRate => Ok(attributes.sample_rate),
            AudioAttributes::Volume => Ok(attributes.volume),
            AudioAttributes::Pan => Ok(attributes.pan),
            AudioAttributes::FXPitch => {
                if !attributes.enable_fx {
                    return Err(AudioPropertyError::AudioFXError(AudioFXError::NotEnabled));
                }

                Ok(attributes.fx_pitch)
            }
            AudioAttributes::FXTempo => {
                if !attributes.enable_fx {
                    return Err(AudioPropertyError::AudioFXError(AudioFXError::NotEnabled));
                }

                Ok(attributes.fx_tempo)
            }
            _ => Err(AudioPropertyError::UnsupportedAttribute(
                "Unknown attribute",
            )),
        }
    }

    fn set_attribute_f32(
        &self,
        _type: AudioAttributes,
        _value: f32,
    ) -> Result<(), AudioPropertyError> {
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
                    return Err(AudioPropertyError::AudioFXError(AudioFXError::NotEnabled));
                }

                attributes.fx_pitch = _value;
                Ok(())
            }
            AudioAttributes::FXTempo => {
                if !attributes.enable_fx {
                    return Err(AudioPropertyError::AudioFXError(AudioFXError::NotEnabled));
                }

                attributes.fx_tempo = _value;
                Ok(())
            }
            _ => Err(AudioPropertyError::UnsupportedAttribute(
                "Unknown attribute",
            )),
        }
    }

    fn get_attribute_bool(&self, _type: AudioAttributes) -> Result<bool, AudioPropertyError> {
        let attributes = self.attributes.lock().unwrap();

        match _type {
            AudioAttributes::AudioFX => Ok(attributes.enable_fx),
            AudioAttributes::AudioSpatialization => Ok(attributes.enable_spatialization),
            _ => Err(AudioPropertyError::UnsupportedAttribute(
                "Unknown attribute",
            )),
        }
    }

    fn set_attribute_bool(
        &self,
        _type: AudioAttributes,
        _value: bool,
    ) -> Result<(), AudioPropertyError> {
        let mut attributes = self.attributes.lock().unwrap();

        match _type {
            AudioAttributes::AudioFX => {
                attributes.enable_fx = _value;
                Ok(())
            }
            AudioAttributes::AudioSpatialization => {
                attributes.enable_spatialization = _value;
                Ok(())
            }
            _ => Err(AudioPropertyError::UnsupportedAttribute(
                "Unknown attribute",
            )),
        }
    }
}

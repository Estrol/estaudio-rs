use crate::{
    channel::{AudioChannel, AudioChannelError},
    device::{
        AudioAttributes, AudioDevice, AudioDeviceError, AudioPropertyError, AudioPropertyHandler,
    },
};

use super::AudioBufferDesc;

#[derive(Debug)]
pub enum AudioChannelBuilderError {
    NoFileOrBufferProvided,
    AudioDeviceError(AudioDeviceError),
    AudioChannelError(AudioChannelError),
    AudioPropertyError(AudioPropertyError),
}

impl std::fmt::Display for AudioChannelBuilderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioChannelBuilderError::NoFileOrBufferProvided => {
                write!(f, "No file or buffer provided for audio channel")
            }
            AudioChannelBuilderError::AudioDeviceError(err) => write!(f, "Audio device error: {}", err),
            AudioChannelBuilderError::AudioChannelError(err) => write!(f, "Audio channel error: {}", err),
            AudioChannelBuilderError::AudioPropertyError(err) => write!(f, "Audio property error: {}", err),
        }
    }
}

/// A builder for creating audio channels.
pub struct AudioChannelBuilder<'a> {
    pub device: Option<&'a mut AudioDevice>,
    pub file_path: Option<String>,
    pub file_buffer: Option<&'a [u8]>,
    pub audio_buffer: Option<AudioBufferDesc<'a>>,
    pub enable_fx: bool,
    pub enable_spatialization: bool,
}

impl<'a> AudioChannelBuilder<'a> {
    pub(crate) fn new() -> Self {
        Self {
            device: None,
            file_path: None,
            file_buffer: None,
            audio_buffer: None,
            enable_fx: false,
            enable_spatialization: false,
        }
    }

    /// Create a new audio channel from a file path.
    pub fn file(mut self, file_path: &str) -> Self {
        self.file_path = Some(file_path.to_string());
        self.file_buffer = None;
        self.audio_buffer = None;
        self
    }

    /// Create a new audio channel from a file buffer.
    pub fn file_buffer(mut self, buffer: &'a [u8]) -> Self {
        self.file_buffer = Some(buffer);
        self.file_path = None;
        self.audio_buffer = None;
        self
    }

    /// Create a new audio buffer from raw PCM data.
    pub fn audio_buffer(mut self, buffer: AudioBufferDesc<'a>) -> Self {
        self.audio_buffer = Some(buffer);
        self.file_path = None;
        self.file_buffer = None;
        self
    }

    /// Auto attach the audio channel to the device.
    pub fn device(mut self, device: &'a mut AudioDevice) -> Self {
        self.device = Some(device);
        self
    }

    /// Enable AudioFX, this is for time stretching and pitch shifting.
    ///
    /// This will enable [AudioAttributes::AudioFX] on the device.
    pub fn enable_fx(mut self, enable: bool) -> Self {
        self.enable_fx = enable;
        self
    }

    /// Enable spatialization, this is useful for 3D audio.
    ///
    /// This will enable [AudioAttributes::AudioSpatialization] on the device.
    pub fn enable_spatialization(mut self, enable: bool) -> Self {
        self.enable_spatialization = enable;
        self
    }

    /// Construct the audio channel.
    pub fn build(self) -> Result<AudioChannel, AudioChannelBuilderError> {
        let channel = if let Some(file_path) = self.file_path {
            AudioChannel::new_file(&file_path)
                .map_err(AudioChannelBuilderError::AudioChannelError)?
        } else if let Some(buffer) = self.file_buffer {
            AudioChannel::new_file_buffer(&buffer)
                .map_err(AudioChannelBuilderError::AudioChannelError)?
        } else if let Some(audio_buffer) = self.audio_buffer {
            AudioChannel::new_audio_buffer(
                &audio_buffer.buffer,
                audio_buffer.pcm_length,
                audio_buffer.sample_rate,
                audio_buffer.channels,
            )
            .map_err(AudioChannelBuilderError::AudioChannelError)?
        } else {
            return Err(AudioChannelBuilderError::NoFileOrBufferProvided);
        };

        channel
            .set_attribute_bool(
                AudioAttributes::AudioSpatialization,
                self.enable_spatialization,
            )
            .map_err(AudioChannelBuilderError::AudioPropertyError)?;
        channel
            .set_attribute_bool(AudioAttributes::AudioFX, self.enable_fx)
            .map_err(AudioChannelBuilderError::AudioPropertyError)?;

        if let Some(device) = self.device {
            device
                .add_channel(&channel)
                .map_err(AudioChannelBuilderError::AudioDeviceError)?;
        }

        Ok(channel)
    }
}

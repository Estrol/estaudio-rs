use crate::{
    channel::AudioChannel,
    device::{AudioAttributes, AudioDevice, AudioPropertyHandler},
};

use super::AudioBufferDesc;

pub struct AudioChannelBuilder<'a> {
    pub device: Option<&'a AudioDevice>,
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
    pub fn device(mut self, device: &'a AudioDevice) -> Self {
        self.device = Some(device);
        self
    }

    /// Enable additional effects using [signalsmitch-strech](https://github.com/Signalsmith-Audio/signalsmith-stretch), this may introduce latency to the audio channel.
    pub fn enable_fx(mut self, enable: bool) -> Self {
        self.enable_fx = enable;
        self
    }

    /// Enable spatialization for the audio channel.
    pub fn enable_spatialization(mut self, enable: bool) -> Self {
        self.enable_spatialization = enable;
        self
    }

    pub fn build(self) -> Result<AudioChannel, String> {
        let channel = if let Some(file_path) = self.file_path {
            AudioChannel::new_file(&file_path)?
        } else if let Some(buffer) = self.file_buffer {
            AudioChannel::new_file_buffer(&buffer)?
        } else if let Some(audio_buffer) = self.audio_buffer {
            AudioChannel::new_audio_buffer(
                &audio_buffer.buffer,
                audio_buffer.pcm_length,
                audio_buffer.sample_rate,
                audio_buffer.channels,
            )?
        } else {
            return Err("No file path or buffer provided".to_string());
        };

        channel.set_attribute_bool(
            AudioAttributes::AudioSpatialization,
            self.enable_spatialization,
        )?;
        channel.set_attribute_bool(AudioAttributes::AudioFX, self.enable_fx)?;

        if let Some(device) = self.device {
            device.add_channel(&channel)?;
        }

        Ok(channel)
    }
}

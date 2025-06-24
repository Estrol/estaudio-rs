use crate::{
    device::{AudioAttributes, AudioPropertyError, AudioPropertyHandler},
    sample::{AudioSample, AudioSampleError},
};

use super::AudioBufferDesc;

#[derive(Debug, Clone)]
pub enum AudioSampleBuilderError {
    NoFileOrBufferProvided,
    AudioSampleError(AudioSampleError),
    AudioPropertyError(AudioPropertyError),
}

impl std::fmt::Display for AudioSampleBuilderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioSampleBuilderError::NoFileOrBufferProvided => {
                write!(f, "No file or buffer provided for audio sample")
            }
            AudioSampleBuilderError::AudioSampleError(err) => write!(f, "Audio sample error: {}", err),
            AudioSampleBuilderError::AudioPropertyError(err) => write!(f, "Audio property error: {}", err),
        }
    }
}

/// A builder for creating audio samples.
pub struct AudioSampleBuilder<'a> {
    pub enable_fx: bool,
    pub enable_spatialization: bool,

    pub file: Option<String>,
    pub buffer: Option<Vec<u8>>,

    pub audio_buffer_desc: Option<AudioBufferDesc<'a>>,
}

impl<'a> AudioSampleBuilder<'a> {
    pub(crate) fn new() -> Self {
        AudioSampleBuilder {
            enable_fx: false,
            enable_spatialization: false,
            file: None,
            buffer: None,
            audio_buffer_desc: None,
        }
    }

    /// Select the file to load the audio sample from.
    pub fn file(mut self, file: &str) -> Self {
        self.file = Some(file.to_string());
        self.audio_buffer_desc = None;
        self.buffer = None;
        self
    }

    /// Select the file buffer to load the audio sample from.
    pub fn buffer(mut self, buffer: &[u8]) -> Self {
        self.buffer = Some(buffer.to_vec());
        self.audio_buffer_desc = None;
        self.file = None;
        self
    }

    /// Select the audio buffer to load the audio sample from.
    /// This is useful for loading audio samples from raw PCM data.
    ///
    /// This using the [AudioBufferDesc] struct to describe the audio buffer.
    pub fn audio_buffer_desc(mut self, audio_buffer_desc: AudioBufferDesc<'a>) -> Self {
        self.audio_buffer_desc = Some(audio_buffer_desc);
        self.buffer = None;
        self.file = None;
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

    /// Construct the audio sample.
    pub fn build(self) -> Result<AudioSample, AudioSampleBuilderError> {
        if self.file.is_none() && self.buffer.is_none() && self.audio_buffer_desc.is_none() {
            return Err(AudioSampleBuilderError::NoFileOrBufferProvided);
        }

        let sample;

        if let Some(file) = self.file {
            sample = AudioSample::load(&file).map_err(AudioSampleBuilderError::AudioSampleError)?;
        } else if let Some(buffer) = self.buffer {
            sample = AudioSample::load_file_buffer(&buffer)
                .map_err(AudioSampleBuilderError::AudioSampleError)?;
        } else if let Some(audio_buffer_desc) = self.audio_buffer_desc {
            sample = AudioSample::load_audio_buffer(
                &audio_buffer_desc.buffer,
                audio_buffer_desc.pcm_length,
                audio_buffer_desc.sample_rate,
                audio_buffer_desc.channels,
            )
            .map_err(AudioSampleBuilderError::AudioSampleError)?;
        } else {
            return Err(AudioSampleBuilderError::NoFileOrBufferProvided);
        }

        sample
            .set_attribute_bool(AudioAttributes::AudioFX, self.enable_fx)
            .map_err(AudioSampleBuilderError::AudioPropertyError)?;

        sample
            .set_attribute_bool(
                AudioAttributes::AudioSpatialization,
                self.enable_spatialization,
            )
            .map_err(AudioSampleBuilderError::AudioPropertyError)?;

        Ok(sample)
    }
}

use crate::{
    device::{AudioAttributes, AudioPropertyHandler},
    sample::AudioSample,
};

use super::AudioBufferDesc;

pub struct AudioSampleBuilder<'a> {
    pub enable_fx: bool,
    pub enable_spatialization: bool,

    pub file: Option<String>,
    pub buffer: Option<Vec<u8>>,

    pub audio_buffer_desc: Option<AudioBufferDesc<'a>>,
}

impl<'a> AudioSampleBuilder<'a> {
    pub fn new() -> Self {
        AudioSampleBuilder {
            enable_fx: false,
            enable_spatialization: false,
            file: None,
            buffer: None,
            audio_buffer_desc: None,
        }
    }

    pub fn file(mut self, file: &str) -> Self {
        self.file = Some(file.to_string());
        self.audio_buffer_desc = None;
        self.buffer = None;
        self
    }

    pub fn buffer(mut self, buffer: &[u8]) -> Self {
        self.buffer = Some(buffer.to_vec());
        self.audio_buffer_desc = None;
        self.file = None;
        self
    }

    pub fn audio_buffer_desc(mut self, audio_buffer_desc: AudioBufferDesc<'a>) -> Self {
        self.audio_buffer_desc = Some(audio_buffer_desc);
        self.buffer = None;
        self.file = None;
        self
    }

    pub fn enable_fx(mut self, enable: bool) -> Self {
        self.enable_fx = enable;
        self
    }

    pub fn enable_spatialization(mut self, enable: bool) -> Self {
        self.enable_spatialization = enable;
        self
    }

    pub fn build(self) -> Result<AudioSample, String> {
        let sample;

        if let Some(file) = self.file {
            sample = AudioSample::load(&file)?;
        } else if let Some(buffer) = self.buffer {
            sample = AudioSample::load_file_buffer(&buffer)?;
        } else if let Some(audio_buffer_desc) = self.audio_buffer_desc {
            sample = AudioSample::load_audio_buffer(
                &audio_buffer_desc.buffer,
                audio_buffer_desc.pcm_length,
                audio_buffer_desc.sample_rate,
                audio_buffer_desc.channels,
            );
        } else {
            return Err("No file or buffer provided".to_string());
        }

        sample.set_attribute_bool(AudioAttributes::AudioFX, self.enable_fx)?;
        sample.set_attribute_bool(
            AudioAttributes::AudioSpatialization,
            self.enable_spatialization,
        )?;

        Ok(sample)
    }
}

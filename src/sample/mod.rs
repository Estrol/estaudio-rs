use std::sync::{Arc, Mutex};

use crate::{
    channel::AudioChannel,
    device::{AudioAttributes, AudioDevice, AudioPropertyHandler, audioreader::AudioReader},
};

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

pub struct AudioSample {
    pub(crate) buffer: Vec<f32>,
    pub(crate) pcm_length: u64,
    pub(crate) sample_rate: u32,
    pub(crate) channels: u32,
    pub(crate) attributes: Arc<Mutex<AudioSampleAttributes>>,
}

impl AudioSample {
    pub(crate) fn load(file_path: &str) -> Result<Self, String> {
        let mut audioreader = AudioReader::load(file_path)?;

        let mut buffer = vec![0.0; audioreader.pcm_length as usize * audioreader.channels as usize];
        audioreader.read(&mut buffer, audioreader.pcm_length)?;

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

    pub(crate) fn load_file_buffer(buffer: &[u8]) -> Result<Self, String> {
        let mut audioreader = AudioReader::load_file_buffer(buffer)?;

        let mut audio_buffer =
            vec![0.0; audioreader.pcm_length as usize * audioreader.channels as usize];
        audioreader.read(&mut audio_buffer, audioreader.pcm_length)?;

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
    ) -> Self {
        let mut attributes = AudioSampleAttributes::default();
        attributes.sample_rate = sample_rate as f32;

        Self {
            buffer: buffer.to_vec(),
            pcm_length,
            sample_rate,
            channels,
            attributes: Arc::new(Mutex::new(attributes)),
        }
    }

    pub fn play(&self, device: &AudioDevice) -> Result<(), String> {
        let channel = AudioChannel::new_audio_buffer(
            &self.buffer,
            self.pcm_length,
            self.sample_rate,
            self.channels,
        )?;

        channel.attach(device)?;
        self.apply_attributes(&channel)?;

        channel.play()?;

        Ok(())
    }

    pub fn get_channels(
        &self,
        device: &AudioDevice,
        size: u32,
    ) -> Result<Vec<AudioChannel>, String> {
        let mut channels = vec![];

        for _ in 0..size {
            let channel = AudioChannel::new_audio_buffer(
                &self.buffer,
                self.pcm_length,
                self.sample_rate,
                self.channels,
            )?;

            channel.attach(&device)?;
            self.apply_attributes(&channel)?;

            channels.push(channel);
        }

        Ok(channels)
    }

    fn apply_attributes(&self, channel: &AudioChannel) -> Result<(), String> {
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
    fn get_attribute_f32(&self, _type: AudioAttributes) -> Result<f32, String> {
        let attributes = self.attributes.lock().unwrap();

        match _type {
            AudioAttributes::Unknown => Err("Unknown attribute".to_string()),
            AudioAttributes::SampleRate => Ok(attributes.sample_rate),
            AudioAttributes::Volume => Ok(attributes.volume),
            AudioAttributes::Pan => Ok(attributes.pan),
            AudioAttributes::FXPitch => {
                if !attributes.enable_fx {
                    return Err("FX is not enabled".to_string());
                }

                Ok(attributes.fx_pitch)
            }
            AudioAttributes::FXTempo => {
                if !attributes.enable_fx {
                    return Err("FX is not enabled".to_string());
                }

                Ok(attributes.fx_tempo)
            }
            AudioAttributes::AudioFX => Err("Unsupported attribute".to_string()),
            AudioAttributes::AudioSpatialization => Err("Unsupported attribute".to_string()),
        }
    }

    fn set_attribute_f32(&self, _type: AudioAttributes, _value: f32) -> Result<(), String> {
        let mut attributes = self.attributes.lock().unwrap();

        match _type {
            AudioAttributes::Unknown => Err("Unknown attribute".to_string()),
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
                    return Err("FX is not enabled".to_string());
                }

                attributes.fx_pitch = _value;
                Ok(())
            }
            AudioAttributes::FXTempo => {
                if !attributes.enable_fx {
                    return Err("FX is not enabled".to_string());
                }

                attributes.fx_tempo = _value;
                Ok(())
            }
            AudioAttributes::AudioFX => Err("Unsupported attribute".to_string()),
            AudioAttributes::AudioSpatialization => Err("Unsupported attribute".to_string()),
        }
    }

    fn get_attribute_bool(&self, _type: AudioAttributes) -> Result<bool, String> {
        let attributes = self.attributes.lock().unwrap();

        match _type {
            AudioAttributes::Unknown => Err("Unknown attribute".to_string()),
            AudioAttributes::SampleRate => Err("Unsupported attribute".to_string()),
            AudioAttributes::Volume => Err("Unsupported attribute".to_string()),
            AudioAttributes::Pan => Err("Unsupported attribute".to_string()),
            AudioAttributes::FXPitch => Err("Unsupported attribute".to_string()),
            AudioAttributes::FXTempo => Err("Unsupported attribute".to_string()),
            AudioAttributes::AudioFX => Ok(attributes.enable_fx),
            AudioAttributes::AudioSpatialization => Ok(attributes.enable_spatialization),
        }
    }

    fn set_attribute_bool(&self, _type: AudioAttributes, _value: bool) -> Result<(), String> {
        let mut attributes = self.attributes.lock().unwrap();

        match _type {
            AudioAttributes::Unknown => Err("Unknown attribute".to_string()),
            AudioAttributes::SampleRate => Err("Unsupported attribute".to_string()),
            AudioAttributes::Volume => Err("Unsupported attribute".to_string()),
            AudioAttributes::Pan => Err("Unsupported attribute".to_string()),
            AudioAttributes::FXPitch => Err("Unsupported attribute".to_string()),
            AudioAttributes::FXTempo => Err("Unsupported attribute".to_string()),
            AudioAttributes::AudioFX => {
                attributes.enable_fx = _value;
                Ok(())
            }
            AudioAttributes::AudioSpatialization => {
                attributes.enable_spatialization = _value;
                Ok(())
            }
        }
    }
}

use crate::{
    device::{
        AudioAttributes, AudioDevice, AudioDeviceError, AudioPropertyError, AudioPropertyHandler,
    },
    mixer::{AudioMixer, AudioMixerError},
};

#[derive(Debug)]
pub enum AudioMixerBuilderError {
    InvalidChannelCount,
    InvalidSampleRate,
    AudioDeviceError(AudioDeviceError),
    AudioMixerError(AudioMixerError),
    AudioPropertyError(AudioPropertyError),
}

impl std::fmt::Display for AudioMixerBuilderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioMixerBuilderError::InvalidChannelCount => {
                write!(f, "Invalid channel count, must be between 1 and 8")
            }
            AudioMixerBuilderError::InvalidSampleRate => {
                write!(f, "Invalid sample rate, must be between 8000 and 192000")
            }
            AudioMixerBuilderError::AudioDeviceError(err) => write!(f, "Audio device error: {}", err),
            AudioMixerBuilderError::AudioMixerError(err) => write!(f, "Audio mixer error: {}", err),
            AudioMixerBuilderError::AudioPropertyError(err) => write!(f, "Audio property error: {}", err),
        }
    }
}

/// A builder for creating an audio mixer.
pub struct AudioMixerBuilder<'a> {
    pub device: Option<&'a mut AudioDevice>,
    pub channel: u32,
    pub sample_rate: u32,
    pub enable_spatialization: bool,
    pub enable_fx: bool,
}

impl<'a> AudioMixerBuilder<'a> {
    pub(crate) fn new() -> Self {
        Self {
            device: None,
            channel: 2,
            sample_rate: 44100,
            enable_spatialization: false,
            enable_fx: false,
        }
    }

    /// Device to attach the mixer to.
    pub fn device(mut self, device: &'a mut AudioDevice) -> Self {
        self.device = Some(device);
        self
    }

    /// What channel type to use, mono = 1, stereo = 2, quad = 4, etc.
    /// Default is stereo (2).
    pub fn channel(mut self, channel: u32) -> Self {
        self.channel = channel;
        self
    }

    /// The sample rate to use, default is 44100.
    pub fn sample_rate(mut self, sample_rate: u32) -> Self {
        self.sample_rate = sample_rate;
        self
    }

    /// Enable spatialization, this is useful for 3D audio.
    ///
    /// This will enable [AudioAttributes::AudioSpatialization] on the device.
    pub fn enable_spatialization(mut self, enable: bool) -> Self {
        self.enable_spatialization = enable;
        self
    }

    /// Enable AudioFX, this is for time stretching and pitch shifting.
    ///
    /// This will enable [AudioAttributes::AudioFX] on the device.
    pub fn enable_fx(mut self, enable: bool) -> Self {
        self.enable_fx = enable;
        self
    }

    /// Build the audio mixer.
    pub fn build(self) -> Result<AudioMixer, AudioMixerBuilderError> {
        if self.channel < 1 || self.channel > 8 {
            return Err(AudioMixerBuilderError::InvalidChannelCount);
        }

        if self.sample_rate < 8000 || self.sample_rate > 192000 {
            return Err(AudioMixerBuilderError::InvalidSampleRate);
        }

        let mixer = AudioMixer::new(self.channel, self.sample_rate)
            .map_err(AudioMixerBuilderError::AudioMixerError)?;

        if let Some(device) = self.device {
            device
                .add_mixer(&mixer)
                .map_err(AudioMixerBuilderError::AudioDeviceError)?;
        }

        mixer
            .set_attribute_bool(
                AudioAttributes::AudioSpatialization,
                self.enable_spatialization,
            )
            .map_err(AudioMixerBuilderError::AudioPropertyError)?;

        mixer
            .set_attribute_bool(AudioAttributes::AudioFX, self.enable_fx)
            .map_err(AudioMixerBuilderError::AudioPropertyError)?;

        Ok(mixer)
    }
}

use crate::device::{
    AudioAttributes, AudioDevice, AudioDeviceError, AudioPropertyError, AudioPropertyHandler,
    context::AudioHardwareInfo,
};

#[derive(Debug)]
pub enum AudioChannelBuilderError {
    InvalidChannelCount(u32),
    InvalidSampleRate(u32),
    AudioDeviceError(AudioDeviceError),
    AudioPropertyError(AudioPropertyError),
}

impl std::fmt::Display for AudioChannelBuilderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioChannelBuilderError::InvalidChannelCount(count) => {
                write!(f, "Invalid channel count: {}", count)
            }
            AudioChannelBuilderError::InvalidSampleRate(rate) => {
                write!(f, "Invalid sample rate: {}", rate)
            }
            AudioChannelBuilderError::AudioDeviceError(err) => write!(f, "Audio device error: {}", err),
            AudioChannelBuilderError::AudioPropertyError(err) => write!(f, "Audio property error: {}", err),
        }
    }
}

/// A builder for creating an audio device.
pub struct AudioDeviceBuilder<'a> {
    pub channel: u32,
    pub sample_rate: u32,
    pub hardware: Option<&'a AudioHardwareInfo>,
    pub enable_spatialization: bool,
    pub enable_fx: bool,
}

impl<'a> AudioDeviceBuilder<'a> {
    pub(crate) fn new() -> Self {
        AudioDeviceBuilder {
            channel: 2,
            sample_rate: 44100,
            hardware: None,
            enable_spatialization: false,
            enable_fx: false,
        }
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

    /// The hardware info to use, if None, the default device will be used,
    /// this is useful for creating a device with a specific hardware info.
    ///
    /// The hardware info can be obtained using the [est_audio::query_devices()] function.
    pub fn hardware(mut self, hardware: &'a AudioHardwareInfo) -> Self {
        self.hardware = Some(hardware);
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

    /// Construct the audio device.
    pub fn build(self) -> Result<AudioDevice, AudioChannelBuilderError> {
        if self.channel != 1 && self.channel != 2 && self.channel != 4 {
            return Err(AudioChannelBuilderError::InvalidChannelCount(self.channel));
        }

        if self.sample_rate != 44100 && self.sample_rate != 48000 {
            return Err(AudioChannelBuilderError::InvalidSampleRate(
                self.sample_rate,
            ));
        }

        let device = AudioDevice::new(self.hardware, self.channel, self.sample_rate)
            .map_err(AudioChannelBuilderError::AudioDeviceError)?;

        device
            .set_attribute_bool(
                AudioAttributes::AudioSpatialization,
                self.enable_spatialization,
            )
            .map_err(AudioChannelBuilderError::AudioPropertyError)?;

        device
            .set_attribute_bool(AudioAttributes::AudioFX, self.enable_fx)
            .map_err(AudioChannelBuilderError::AudioPropertyError)?;

        Ok(device)
    }
}

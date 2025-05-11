use crate::device::{
    AudioAttributes, AudioDevice, AudioPropertyHandler, context::AudioHardwareInfo,
};

pub struct AudioDeviceBuilder<'a> {
    pub channel: u32,
    pub sample_rate: u32,
    pub hardware: Option<&'a AudioHardwareInfo>,
    pub enable_spatialization: bool,
    pub enable_fx: bool,
}

impl<'a> AudioDeviceBuilder<'a> {
    pub fn new() -> Self {
        AudioDeviceBuilder {
            channel: 2,
            sample_rate: 44100,
            hardware: None,
            enable_spatialization: false,
            enable_fx: false,
        }
    }

    pub fn channel(mut self, channel: u32) -> Self {
        self.channel = channel;
        self
    }

    pub fn sample_rate(mut self, sample_rate: u32) -> Self {
        self.sample_rate = sample_rate;
        self
    }

    pub fn hardware(mut self, hardware: &'a AudioHardwareInfo) -> Self {
        self.hardware = Some(hardware);
        self
    }

    pub fn enable_spatialization(mut self, enable: bool) -> Self {
        self.enable_spatialization = enable;
        self
    }

    pub fn enable_fx(mut self, enable: bool) -> Self {
        self.enable_fx = enable;
        self
    }

    pub fn build(self) -> Result<AudioDevice, String> {
        let device = AudioDevice::new(self.hardware, self.channel, self.sample_rate)?;

        device.set_attribute_bool(
            AudioAttributes::AudioSpatialization,
            self.enable_spatialization,
        )?;
        device.set_attribute_bool(AudioAttributes::AudioFX, self.enable_fx)?;

        Ok(device)
    }
}

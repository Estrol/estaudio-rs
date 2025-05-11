use crate::{device::AudioDevice, mixer::AudioMixer};

pub struct AudioMixerBuilder<'a> {
    pub device: Option<&'a AudioDevice>,
    pub channel: u32,
    pub sample_rate: u32,
}

impl<'a> AudioMixerBuilder<'a> {
    pub fn new() -> Self {
        Self {
            device: None,
            channel: 2,
            sample_rate: 44100,
        }
    }

    pub fn device(mut self, device: &'a AudioDevice) -> Self {
        self.device = Some(device);
        self
    }

    pub fn channel(mut self, channel: u32) -> Self {
        self.channel = channel;
        self
    }

    pub fn sample_rate(mut self, sample_rate: u32) -> Self {
        self.sample_rate = sample_rate;
        self
    }

    pub fn build(self) -> Result<AudioMixer, String> {
        let mixer = AudioMixer::new(self.channel, self.sample_rate)?;

        if let Some(device) = self.device {
            device.add_mixer(&mixer)?;
        }

        Ok(mixer)
    }
}

#![allow(dead_code)]

use device::{AudioDevice, context::AudioHardwareInfo};

pub(crate) mod effects;
pub(crate) mod utils;

pub use utils::PCMIndex;

pub mod builders;
pub mod channel;
pub mod device;
pub mod mixer;
pub mod sample;

use builders::*;

pub struct AudioEngine;

impl AudioEngine {
    pub fn make_device(hardware: Option<&AudioHardwareInfo>) -> AudioDeviceBuilder<'_> {
        let mut builder = AudioDeviceBuilder::new();

        if hardware.is_some() {
            builder = builder.hardware(hardware.unwrap());
        }

        builder
    }

    pub fn query_devices() -> Result<Vec<AudioHardwareInfo>, String> {
        AudioDevice::enumerable()
    }

    pub fn make_channel(device: Option<&AudioDevice>) -> AudioChannelBuilder<'_> {
        let mut builder = AudioChannelBuilder::new();

        if device.is_some() {
            builder = builder.device(device.unwrap());
        }

        builder
    }

    pub fn make_sample() -> AudioSampleBuilder<'static> {
        AudioSampleBuilder::new()
    }

    pub fn make_mixer() -> AudioMixerBuilder<'static> {
        AudioMixerBuilder::new()
    }
}

#[allow(unused_imports)]
pub mod prelude {
    pub use super::*;
    pub use crate::channel::*;
    pub use crate::device::*;
    pub use crate::mixer::*;
    pub use crate::sample::*;
}

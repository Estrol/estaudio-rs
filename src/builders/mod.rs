//! Helpers for building audio objects.

pub struct AudioBufferDesc<'a> {
    pub buffer: &'a [f32],
    pub pcm_length: u64,
    pub sample_rate: u32,
    pub channels: u32,
}

mod channel;
mod device;
mod mixer;
mod sample;

pub use channel::AudioChannelBuilder;
pub use device::AudioDeviceBuilder;
pub use mixer::AudioMixerBuilder;
pub use sample::AudioSampleBuilder;

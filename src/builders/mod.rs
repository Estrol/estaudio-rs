pub struct AudioBufferDesc<'a> {
    pub buffer: &'a [f32],
    pub pcm_length: u64,
    pub sample_rate: u32,
    pub channels: u32,
}

pub mod channel;
pub mod device;
pub mod mixer;
pub mod sample;

pub use channel::AudioChannelBuilder;
pub use device::AudioDeviceBuilder;
pub use mixer::AudioMixerBuilder;
pub use sample::AudioSampleBuilder;

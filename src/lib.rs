//! Yet another rust audio library built with miniaudio and signalsmitch-stretch.

use device::{AudioDevice, context::AudioHardwareInfo};

pub(crate) mod effects;
pub(crate) mod utils;

pub use effects::{AudioSpartialListenerHandler, AudioSpatializationHandler};
pub use utils::PCMIndex;

pub mod builders;
pub mod channel;
pub mod device;
pub mod mixer;
pub mod sample;

#[cfg(feature = "capi")]
pub(crate) mod capi;

use builders::*;

use crate::device::AudioDeviceError;

/// Constructs a new audio device builder.
///
/// If a hardware info is provided, it will be used to create the device.
/// Otherwise, a default device will be created.
///
/// The builder can be further configured with various options.
/// Finally, the build() method will create the device.
pub fn create_device(hardware: Option<&AudioHardwareInfo>) -> AudioDeviceBuilder<'_> {
    let mut builder = AudioDeviceBuilder::new();

    if hardware.is_some() {
        builder = builder.hardware(hardware.unwrap());
    }

    builder
}

/// Queries the available audio devices on the system.
///
/// This function returns a vector of AudioHardwareInfo structs,
/// each representing an audio device.
pub fn query_devices() -> Result<Vec<AudioHardwareInfo>, AudioDeviceError> {
    AudioDevice::enumerable()
}

/// Constructs a new audio channel builder.
///
/// This function takes an optional AudioDevice reference.
/// If provided, the channel will be associated with that device, else
/// the channel will be created without a device and can be added to a device later.
pub fn create_channel(device: Option<&mut AudioDevice>) -> AudioChannelBuilder<'_> {
    let mut builder = AudioChannelBuilder::new();

    if device.is_some() {
        builder = builder.device(device.unwrap());
    }

    builder
}

/// Constructs a new audio sample builder for quickly creating channels without
/// consuming a lot of memory.
pub fn create_sample() -> AudioSampleBuilder<'static> {
    AudioSampleBuilder::new()
}

/// Constructs a new audio mixer builder which can be used to create channel mixers
/// or even the audio mixer itself.
///
/// This function takes an optional AudioDevice reference which can be used to
/// associate the mixer with a specific device.
/// If no device is provided, the mixer will be created without a device and can
/// be added to a device later.
pub fn create_mixer(device: Option<&mut AudioDevice>) -> AudioMixerBuilder<'_> {
    let mut builder = AudioMixerBuilder::new();

    if device.is_some() {
        builder = builder.device(device.unwrap());
    }

    builder
}

#[allow(unused_imports)]
pub mod prelude {
    pub use crate::channel::*;
    pub use crate::device::*;
    pub use crate::mixer::*;
    pub use crate::sample::*;
}

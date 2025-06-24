use std::sync::{Arc, Mutex};

use inner::AudioDeviceInner;

use crate::{
    channel::{AudioChannel, AudioChannelError},
    effects::{
        AudioFX, AudioFXError, AudioPannerError, AudioResamplerError, AudioSpartialListenerHandler,
        AudioSpatializationError, AudioSpatializationListener, AudioSpatializationListenerError,
        AudioVolumeError,
    },
    mixer::AudioMixer,
    utils::{self, MutexPoison},
};

pub(crate) mod audioreader;
pub(crate) mod context;
pub(crate) mod inner;

use context::*;

pub type AudioDeviceDSPCallback = fn(buffer: &[f32], frame_count: u64);

pub enum AudioAttributes {
    Unknown,
    /// The sample rate of the audio channel, device or mixer.
    SampleRate,
    /// The volume of the audio channel, device or mixer.
    Volume,
    /// The pan of the audio channel, device or mixer.
    Pan,
    /// The pitch of the audio channel. \
    /// This require the [AudioAttributes::AudioFX] on [AudioDevice] to be enabled.
    FXPitch,
    /// The tempo of the audio channel. \
    /// This require the [AudioAttributes::AudioFX] on [AudioDevice] to be enabled.
    FXTempo,
    /// Enable or disable the AudioFX used for Tempo and Pitch on the audio channel, device or mixer.
    AudioFX,
    /// Enable or disable the AudioSpatialization used for 3D Audio on the audio channel, device or mixer.
    AudioSpatialization,
}

impl AudioAttributes {
    pub fn from(name: &str) -> Self {
        match name {
            "SampleRate" => AudioAttributes::SampleRate,
            "Volume" => AudioAttributes::Volume,
            "Pan" => AudioAttributes::Pan,
            "FXPitch" => AudioAttributes::FXPitch,
            "FXTempo" => AudioAttributes::FXTempo,
            _ => AudioAttributes::Unknown,
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            AudioAttributes::SampleRate => "SampleRate".to_string(),
            AudioAttributes::Volume => "Volume".to_string(),
            AudioAttributes::Pan => "Pan".to_string(),
            AudioAttributes::FXPitch => "FXPitch".to_string(),
            AudioAttributes::FXTempo => "FXTempo".to_string(),
            AudioAttributes::AudioFX => "AudioFX".to_string(),
            AudioAttributes::AudioSpatialization => "AudioSpatialization".to_string(),
            AudioAttributes::Unknown => "Unknown".to_string(),
        }
    }
}

pub trait AudioPropertyHandler {
    /// Get the [AudioAttributes] value (f32) of the [AudioChannel], [AudioDevice] or [AudioMixer].
    fn get_attribute_f32(&self, _type: AudioAttributes) -> Result<f32, AudioPropertyError>;
    /// Set the [AudioAttributes] value (f32) of the [AudioChannel], [AudioDevice] or [AudioMixer].
    fn set_attribute_f32(
        &self,
        _type: AudioAttributes,
        _value: f32,
    ) -> Result<(), AudioPropertyError>;
    /// Get the [AudioAttributes] value (bool) of the [AudioChannel], [AudioDevice] or [AudioMixer].
    fn get_attribute_bool(&self, _type: AudioAttributes) -> Result<bool, AudioPropertyError>;
    /// Set the [AudioAttributes] value (bool) of the [AudioChannel], [AudioDevice] or [AudioMixer].
    fn set_attribute_bool(
        &self,
        _type: AudioAttributes,
        _value: bool,
    ) -> Result<(), AudioPropertyError>;
}

#[derive(Debug, Clone)]
pub enum AudioPropertyError {
    UnsupportedAttribute(&'static str),
    AudioFXError(AudioFXError),
    SpatializationListenerError(AudioSpatializationListenerError),
    AudioChannelError(AudioChannelError),
    AudioSpatializationListenerError(AudioSpatializationListenerError),
    AudioSpatializationError(AudioSpatializationError),
}

impl std::fmt::Display for AudioPropertyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioPropertyError::UnsupportedAttribute(attr) => {
                write!(f, "Unsupported attribute: {}", attr)
            }
            AudioPropertyError::AudioFXError(e) => write!(f, "AudioFX error: {}", e),
            AudioPropertyError::SpatializationListenerError(e) => {
                write!(f, "Spatialization listener error: {}", e)
            }
            AudioPropertyError::AudioChannelError(e) => write!(f, "Audio channel error: {}", e),
            AudioPropertyError::AudioSpatializationListenerError(e) => {
                write!(f, "Audio spatialization listener error: {}", e)
            }
            AudioPropertyError::AudioSpatializationError(e) => {
                write!(f, "Audio spatialization error: {}", e)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum AudioDeviceError {
    InitializationError(i32),
    InvalidChannels,
    InvalidSampleRate,
    InvalidOperation(i32),
    ChannelNotFound(usize),
    MixerNotFound(usize),
    ChannelAlreadyExists(usize),
    MixerAlreadyExists(usize),
    AudioChannelError(AudioChannelError),
    AudioContextError(AudioContextError),
    AudioSpatializationListenerError(AudioSpatializationListenerError),
    AudioFXError(AudioFXError),
    AudioVolumeError(AudioVolumeError),
    AudioPannerError(AudioPannerError),
    AudioResamplerError(AudioResamplerError),
    AudioPropertyError(AudioPropertyError),
}

impl std::fmt::Display for AudioDeviceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioDeviceError::InitializationError(code) => write!(
                f,
                "Failed to initialize audio device: {} ({})",
                code,
                utils::ma_to_string_result(*code)
            ),
            AudioDeviceError::InvalidChannels => write!(f, "Invalid number of channels"),
            AudioDeviceError::InvalidSampleRate => write!(f, "Invalid sample rate"),
            AudioDeviceError::InvalidOperation(code) => write!(
                f,
                "Invalid operation with code: {} ({})",
                code,
                utils::ma_to_string_result(*code)
            ),
            AudioDeviceError::ChannelNotFound(ref_id) => {
                write!(f, "Audio channel with reference id {} not found", ref_id)
            }
            AudioDeviceError::MixerNotFound(ref_id) => {
                write!(f, "Audio mixer with reference id {} not found", ref_id)
            }
            AudioDeviceError::ChannelAlreadyExists(ref_id) => write!(
                f,
                "Audio channel with reference id {} already exists",
                ref_id
            ),
            AudioDeviceError::MixerAlreadyExists(ref_id) => {
                write!(f, "Audio mixer with reference id {} already exists", ref_id)
            }
            AudioDeviceError::AudioChannelError(e) => write!(f, "Audio channel error: {}", e),
            AudioDeviceError::AudioContextError(e) => write!(f, "Audio context error: {}", e),
            AudioDeviceError::AudioSpatializationListenerError(e) => {
                write!(f, "Audio spatialization listener error: {}", e)
            }
            AudioDeviceError::AudioFXError(e) => write!(f, "Audio FX error: {}", e),
            AudioDeviceError::AudioVolumeError(e) => write!(f, "Audio volume error: {}", e),
            AudioDeviceError::AudioPannerError(e) => write!(f, "Audio panner error: {}", e),
            AudioDeviceError::AudioResamplerError(e) => write!(f, "Audio resampler error: {}", e),
            AudioDeviceError::AudioPropertyError(e) => write!(f, "Audio property error: {}", e),
        }
    }
}

/// A hardware audio device, used to play audio comes from Channel and Mixer.
pub struct AudioDevice {
    pub(crate) inner: Arc<Mutex<Box<AudioDeviceInner>>>,

    // Used for lifetime management of the hardware context
    #[allow(dead_code)]
    pub(crate) hardware: Option<AudioHardwareInfo>,
}

impl AudioDevice {
    pub(crate) fn enumerable() -> Result<Vec<AudioHardwareInfo>, AudioDeviceError> {
        let context = AudioContext::new().map_err(AudioDeviceError::AudioContextError)?;

        let devices = enumerable(context).map_err(AudioDeviceError::AudioContextError)?;

        Ok(devices)
    }

    pub(crate) fn new(
        hardware: Option<&AudioHardwareInfo>,
        channels: u32,
        sample_rate: u32,
    ) -> Result<Self, AudioDeviceError> {
        let inner = AudioDeviceInner::new(hardware, channels, sample_rate)?;

        Ok({
            AudioDevice {
                inner: Arc::new(Mutex::new(inner)),
                hardware: hardware.cloned(),
            }
        })
    }

    /// Add [AudioChannel] to the device.
    pub fn add_channel(&mut self, channel: &AudioChannel) -> Result<(), AudioDeviceError> {
        let mut inner = self.inner.lock_poison();
        inner.add_channel(channel.inner.clone())?;

        Ok(())
    }

    /// Remove [AudioChannel] from the device.
    pub fn remove_channel(&mut self, channel: &AudioChannel) -> Result<(), AudioDeviceError> {
        let mut inner = self.inner.lock_poison();
        inner.remove_channel(channel.ref_id())?;

        Ok(())
    }

    /// Remove [AudioChannel] from the device by reference id which frok [AudioChannel::ref_id()].
    pub fn remove_channel_by_ref(&mut self, ref_id: usize) -> Result<(), AudioDeviceError> {
        let mut inner = self.inner.lock_poison();
        inner.remove_channel(ref_id)?;

        Ok(())
    }

    /// Add [AudioMixer] to the device.
    pub fn add_mixer(&mut self, mixer: &AudioMixer) -> Result<(), AudioDeviceError> {
        let mut inner = self.inner.lock_poison();
        inner.add_mixer(mixer.inner.clone())?;

        Ok(())
    }

    /// Remove [AudioMixer] from the device.
    pub fn remove_mixer(&mut self, mixer: &AudioMixer) -> Result<(), AudioDeviceError> {
        let mut inner = self.inner.lock_poison();
        inner.remove_mixer(mixer.ref_id())?;

        Ok(())
    }

    /// Remove [AudioMixer] from the device by reference id which frok [AudioMixer::ref_id()].
    pub fn remove_mixer_by_ref(&mut self, ref_id: usize) -> Result<(), AudioDeviceError> {
        let mut inner = self.inner.lock_poison();
        inner.remove_mixer(ref_id)?;

        Ok(())
    }

    /// Set DSP callback for the device, useful for custom audio processing before
    /// sending the audio to the hardware.
    ///
    /// The buffer is a slice of f32, non-cliped and non-normalized with length frame_count * channels.
    pub fn set_dsp_callback(
        &mut self,
        callback: AudioDeviceDSPCallback,
    ) -> Result<(), AudioDeviceError> {
        // FIXME:
        let mut inner = self.inner.lock().unwrap();

        inner.dsp_callback = Some(callback);

        Ok(())
    }
}

impl AudioPropertyHandler for AudioDevice {
    fn get_attribute_f32(&self, _type: AudioAttributes) -> Result<f32, AudioPropertyError> {
        let inner = self.inner.lock().unwrap();

        match _type {
            AudioAttributes::Unknown => Err(AudioPropertyError::UnsupportedAttribute(
                "Unknown attribute",
            )),
            AudioAttributes::SampleRate => Ok(inner.resampler.sample_rate as f32),
            AudioAttributes::Volume => Ok(inner.volume.volume),
            AudioAttributes::Pan => Ok(inner.panner.pan),
            AudioAttributes::AudioFX => Err(AudioPropertyError::UnsupportedAttribute(
                "AudioFX is not supported, use set_attribute_bool to enable it",
            )),
            AudioAttributes::AudioSpatialization => Err(AudioPropertyError::UnsupportedAttribute(
                "AudioSpatialization is not supported, use set_attribute_bool to enable it",
            )),
            AudioAttributes::FXPitch => {
                let fx = inner.fx.as_ref();
                if let Some(fx) = fx {
                    Ok(fx.octave)
                } else {
                    Err(AudioPropertyError::AudioFXError(AudioFXError::NotEnabled))
                }
            }
            AudioAttributes::FXTempo => {
                let fx = inner.fx.as_ref();
                if let Some(fx) = fx {
                    Ok(fx.tempo)
                } else {
                    Err(AudioPropertyError::AudioFXError(AudioFXError::NotEnabled))
                }
            }
        }
    }

    fn set_attribute_f32(
        &self,
        _type: AudioAttributes,
        _value: f32,
    ) -> Result<(), AudioPropertyError> {
        let mut inner = self.inner.lock().unwrap();

        match _type {
            AudioAttributes::Unknown => Err(AudioPropertyError::UnsupportedAttribute(
                "Unknown attribute",
            )),
            AudioAttributes::SampleRate => {
                inner.resampler.set_target_sample_rate(_value as u32);
                Ok(())
            }
            AudioAttributes::Volume => {
                inner.volume.set_volume(_value);
                Ok(())
            }
            AudioAttributes::Pan => {
                inner.panner.set_pan(_value);
                Ok(())
            }
            AudioAttributes::AudioFX => Err(AudioPropertyError::UnsupportedAttribute(
                "AudioFX is not supported, use set_attribute_bool to enable it",
            )),
            AudioAttributes::AudioSpatialization => Err(AudioPropertyError::UnsupportedAttribute(
                "AudioSpatialization is not supported, use set_attribute_bool to enable it",
            )),
            AudioAttributes::FXPitch => {
                let fx = inner.fx.as_mut();
                if let Some(fx) = fx {
                    if let Err(e) = fx.set_octave(_value) {
                        return Err(AudioPropertyError::AudioFXError(e));
                    }

                    Ok(())
                } else {
                    Err(AudioPropertyError::AudioFXError(AudioFXError::NotEnabled))
                }
            }
            AudioAttributes::FXTempo => {
                let fx = inner.fx.as_mut();
                if let Some(fx) = fx {
                    if let Err(e) = fx.set_tempo(_value) {
                        return Err(AudioPropertyError::AudioFXError(e));
                    }

                    Ok(())
                } else {
                    Err(AudioPropertyError::AudioFXError(AudioFXError::NotEnabled))
                }
            }
        }
    }

    fn get_attribute_bool(&self, _type: AudioAttributes) -> Result<bool, AudioPropertyError> {
        let inner = self.inner.lock().unwrap();

        match _type {
            AudioAttributes::Unknown => Err(AudioPropertyError::UnsupportedAttribute(
                "Unknown attribute",
            )),
            AudioAttributes::AudioFX => Ok(inner.fx.is_some()),
            AudioAttributes::AudioSpatialization => Ok(inner.spatialization.is_some()),
            _ => Err(AudioPropertyError::UnsupportedAttribute(
                "Unsupported attribute",
            )),
        }
    }

    fn set_attribute_bool(
        &self,
        _type: AudioAttributes,
        _value: bool,
    ) -> Result<(), AudioPropertyError> {
        let mut inner = self.inner.lock().unwrap();

        match _type {
            AudioAttributes::Unknown => Err(AudioPropertyError::UnsupportedAttribute(
                "Unknown attribute",
            )),
            AudioAttributes::AudioFX => {
                if _value {
                    let fx = AudioFX::new(inner.resampler.channels, inner.resampler.sample_rate);

                    if let Err(e) = fx {
                        return Err(AudioPropertyError::AudioFXError(e));
                    }

                    inner.fx = fx.ok();
                } else {
                    inner.fx = None;
                }
                Ok(())
            }
            AudioAttributes::AudioSpatialization => {
                if _value {
                    let spatialization = AudioSpatializationListener::new(inner.resampler.channels);
                    if let Err(e) = spatialization {
                        return Err(AudioPropertyError::SpatializationListenerError(e));
                    }

                    inner.spatialization = spatialization.ok();
                } else {
                    inner.spatialization = None;
                }
                Ok(())
            }
            _ => Err(AudioPropertyError::UnsupportedAttribute(
                "Unsupported attribute",
            )),
        }
    }
}

impl AudioSpartialListenerHandler for AudioDevice {
    fn set_position(&self, x: f32, y: f32, z: f32) -> Result<(), AudioSpatializationListenerError> {
        let mut inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_mut() {
            spatialization.set_position(x, y, z);
            Ok(())
        } else {
            Err(AudioSpatializationListenerError::NotInitialized)
        }
    }

    fn get_position(&self) -> Result<(f32, f32, f32), AudioSpatializationListenerError> {
        let inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_ref() {
            Ok(spatialization.get_position())
        } else {
            Err(AudioSpatializationListenerError::NotInitialized)
        }
    }

    fn set_direction(
        &self,
        x: f32,
        y: f32,
        z: f32,
    ) -> Result<(), AudioSpatializationListenerError> {
        let mut inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_mut() {
            spatialization.set_direction(x, y, z);
            Ok(())
        } else {
            Err(AudioSpatializationListenerError::NotInitialized)
        }
    }

    fn get_direction(&self) -> Result<(f32, f32, f32), AudioSpatializationListenerError> {
        let inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_ref() {
            Ok(spatialization.get_direction())
        } else {
            Err(AudioSpatializationListenerError::NotInitialized)
        }
    }

    fn set_velocity(&self, x: f32, y: f32, z: f32) -> Result<(), AudioSpatializationListenerError> {
        let mut inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_mut() {
            spatialization.set_velocity(x, y, z);
            Ok(())
        } else {
            Err(AudioSpatializationListenerError::NotInitialized)
        }
    }

    fn get_velocity(&self) -> Result<(f32, f32, f32), AudioSpatializationListenerError> {
        let inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_ref() {
            Ok(spatialization.get_velocity())
        } else {
            Err(AudioSpatializationListenerError::NotInitialized)
        }
    }

    fn set_speed_of_sound(&self, speed: f32) -> Result<(), AudioSpatializationListenerError> {
        let mut inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_mut() {
            spatialization.set_speed_of_sound(speed);
            Ok(())
        } else {
            Err(AudioSpatializationListenerError::NotInitialized)
        }
    }

    fn get_speed_of_sound(&self) -> Result<f32, AudioSpatializationListenerError> {
        let inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_ref() {
            Ok(spatialization.get_speed_of_sound())
        } else {
            Err(AudioSpatializationListenerError::NotInitialized)
        }
    }

    fn set_world_up(&self, x: f32, y: f32, z: f32) -> Result<(), AudioSpatializationListenerError> {
        let mut inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_mut() {
            spatialization.set_world_up(x, y, z);
            Ok(())
        } else {
            Err(AudioSpatializationListenerError::NotInitialized)
        }
    }

    fn get_world_up(&self) -> Result<(f32, f32, f32), AudioSpatializationListenerError> {
        let inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_ref() {
            Ok(spatialization.get_world_up())
        } else {
            Err(AudioSpatializationListenerError::NotInitialized)
        }
    }

    fn set_cone(
        &self,
        inner_angle: f32,
        outer_angle: f32,
        outer_gain: f32,
    ) -> Result<(), AudioSpatializationListenerError> {
        let mut inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_mut() {
            spatialization.set_cone(inner_angle, outer_angle, outer_gain);
            Ok(())
        } else {
            Err(AudioSpatializationListenerError::NotInitialized)
        }
    }

    fn get_cone(&self) -> Result<(f32, f32, f32), AudioSpatializationListenerError> {
        let inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_ref() {
            Ok(spatialization.get_cone())
        } else {
            Err(AudioSpatializationListenerError::NotInitialized)
        }
    }

    fn set_enabled(&self, is_enabled: bool) -> Result<(), AudioSpatializationListenerError> {
        let mut inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_mut() {
            spatialization.set_enabled(is_enabled);
            Ok(())
        } else {
            Err(AudioSpatializationListenerError::NotInitialized)
        }
    }

    fn is_enabled(&self) -> Result<bool, AudioSpatializationListenerError> {
        let inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_ref() {
            Ok(spatialization.is_enabled())
        } else {
            Err(AudioSpatializationListenerError::NotInitialized)
        }
    }
}

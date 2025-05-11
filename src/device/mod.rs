use std::sync::{Arc, Mutex};

use inner::AudioDeviceInner;

use crate::{
    channel::AudioChannel,
    effects::{AudioFX, AudioSpartialListenerHandler, AudioSpatializationListener},
    mixer::AudioMixer,
    utils::MutexPoison,
};

pub(crate) mod audioreader;
pub(crate) mod context;
pub(crate) mod inner;

use context::*;

pub type AudioDeviceDSPCallback = fn(buffer: &[f32], frame_count: u64);

pub enum AudioAttributes {
    Unknown,
    /// The sample rate of the audio channel.
    SampleRate,
    /// The volume of the audio channel.
    Volume,
    /// The pan of the audio channel.
    Pan,
    /// The pitch of the audio channel. \
    /// This require the [AudioFX] on [AudioChannelBuilder] to be enabled.
    FXPitch,
    /// The tempo of the audio channel. \
    /// This require the [AudioFX] on [AudioChannelBuilder] to be enabled.
    FXTempo,
    /// Check if the [AudioFX] is enabled on the audio channel.
    AudioFX,
    /// Check if the [AudioSpatialization] is enabled on the audio channel.
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
    /// Get the attribute value (f32) of the audio channel.
    fn get_attribute_f32(&self, _type: AudioAttributes) -> Result<f32, String>;
    /// Set the attribute value (f32) of the audio channel.
    fn set_attribute_f32(&self, _type: AudioAttributes, _value: f32) -> Result<(), String>;
    /// Get the attribute value (bool) of the audio channel.
    fn get_attribute_bool(&self, _type: AudioAttributes) -> Result<bool, String>;
    /// Set the attribute value (bool) of the audio channel.
    fn set_attribute_bool(&self, _type: AudioAttributes, _value: bool) -> Result<(), String>;
}

pub struct AudioDevice {
    pub(crate) inner: Arc<Mutex<Box<AudioDeviceInner>>>,

    // Used for lifetime management of the hardware context
    #[allow(dead_code)]
    pub(crate) hardware: Option<AudioHardwareInfo>,
}

impl AudioDevice {
    pub fn enumerable() -> Result<Vec<AudioHardwareInfo>, String> {
        let context = AudioContext::new()?;
        let devices = enumerable(context)?;

        Ok(devices)
    }

    pub fn new(
        hardware: Option<&AudioHardwareInfo>,
        channels: u32,
        sample_rate: u32,
    ) -> Result<Self, String> {
        let inner = AudioDeviceInner::new(hardware, channels, sample_rate)?;

        Ok({
            AudioDevice {
                inner: Arc::new(Mutex::new(inner)),
                hardware: hardware.cloned(),
            }
        })
    }

    pub fn add_channel(&self, channel: &AudioChannel) -> Result<(), String> {
        let mut inner = self.inner.lock_poison();
        inner.add_channel(channel.inner.clone())?;

        Ok(())
    }

    pub fn remove_channel(&self, channel: &AudioChannel) -> Result<(), String> {
        let mut inner = self.inner.lock_poison();
        inner.remove_channel(channel.ref_id())?;

        Ok(())
    }

    pub fn remove_channel_by_ref(&self, ref_id: usize) -> Result<(), String> {
        let mut inner = self.inner.lock_poison();
        inner.remove_channel(ref_id)?;

        Ok(())
    }

    pub fn add_mixer(&self, mixer: &AudioMixer) -> Result<(), String> {
        let mut inner = self.inner.lock_poison();
        inner.add_mixer(mixer.inner.clone())?;

        Ok(())
    }

    pub fn remove_mixer(&self, mixer: &AudioMixer) -> Result<(), String> {
        let mut inner = self.inner.lock_poison();
        inner.remove_mixer(mixer.ref_id())?;

        Ok(())
    }

    pub fn remove_mixer_by_ref(&self, ref_id: usize) -> Result<(), String> {
        let mut inner = self.inner.lock_poison();
        inner.remove_mixer(ref_id)?;

        Ok(())
    }

    pub fn set_dsp_callback(&self, callback: AudioDeviceDSPCallback) -> Result<(), String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "Failed to lock inner".to_string())?;

        inner.dsp_callback = Some(callback);

        Ok(())
    }
}

impl AudioPropertyHandler for AudioDevice {
    fn get_attribute_f32(&self, _type: AudioAttributes) -> Result<f32, String> {
        let inner = self.inner.lock().unwrap();

        match _type {
            AudioAttributes::Unknown => Err("Unknown attribute".to_string()),
            AudioAttributes::SampleRate => Ok(inner.resampler.sample_rate as f32),
            AudioAttributes::Volume => Ok(inner.volume.volume),
            AudioAttributes::Pan => Ok(inner.panner.pan),
            AudioAttributes::AudioFX => Err("Unsupported attribute".to_string()),
            AudioAttributes::AudioSpatialization => Err("Unsupported attribute".to_string()),
            AudioAttributes::FXPitch => {
                let fx = inner.fx.as_ref();
                if let Some(fx) = fx {
                    Ok(fx.octave)
                } else {
                    Err("AudioFX is not enabled!".to_string())
                }
            }
            AudioAttributes::FXTempo => {
                let fx = inner.fx.as_ref();
                if let Some(fx) = fx {
                    Ok(fx.tempo)
                } else {
                    Err("AudioFX is not enabled!".to_string())
                }
            }
        }
    }

    fn set_attribute_f32(&self, _type: AudioAttributes, _value: f32) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();

        match _type {
            AudioAttributes::Unknown => Err("Unknown attribute".to_string()),
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
            AudioAttributes::AudioFX => Err("Unsupported attribute".to_string()),
            AudioAttributes::AudioSpatialization => Err("Unsupported attribute".to_string()),
            AudioAttributes::FXPitch => {
                let fx = inner.fx.as_mut();
                if let Some(fx) = fx {
                    fx.set_octave(_value)
                } else {
                    Err("AudioFX is not enabled!".to_string())
                }
            }
            AudioAttributes::FXTempo => {
                let fx = inner.fx.as_mut();
                if let Some(fx) = fx {
                    fx.set_tempo(_value)
                } else {
                    Err("AudioFX is not enabled!".to_string())
                }
            }
        }
    }

    fn get_attribute_bool(&self, _type: AudioAttributes) -> Result<bool, String> {
        let inner = self.inner.lock().unwrap();

        match _type {
            AudioAttributes::Unknown => Err("Unknown attribute".to_string()),
            AudioAttributes::AudioFX => Ok(inner.fx.is_some()),
            AudioAttributes::AudioSpatialization => Ok(inner.spatialization.is_some()),
            _ => Err("Unsupported attribute".to_string()),
        }
    }

    fn set_attribute_bool(&self, _type: AudioAttributes, _value: bool) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();

        match _type {
            AudioAttributes::Unknown => Err("Unknown attribute".to_string()),
            AudioAttributes::AudioFX => {
                if _value {
                    inner.fx = Some(AudioFX::new(
                        inner.resampler.channels,
                        inner.resampler.sample_rate,
                    )?);
                } else {
                    inner.fx = None;
                }
                Ok(())
            }
            AudioAttributes::AudioSpatialization => {
                if _value {
                    inner.spatialization = Some(AudioSpatializationListener::new(
                        inner.device.playback.channels,
                    )?);
                } else {
                    inner.spatialization = None;
                }
                Ok(())
            }
            _ => Err("Unsupported attribute".to_string()),
        }
    }
}

impl AudioSpartialListenerHandler for AudioDevice {
    fn set_position(&self, x: f32, y: f32, z: f32) -> Result<(), String> {
        let mut inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_mut() {
            spatialization.set_position(x, y, z);
            Ok(())
        } else {
            Err("Spatialization is not enabled".to_string())
        }
    }

    fn get_position(&self) -> Result<(f32, f32, f32), String> {
        let inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_ref() {
            Ok(spatialization.get_position())
        } else {
            Err("Spatialization is not enabled".to_string())
        }
    }

    fn set_direction(&self, x: f32, y: f32, z: f32) -> Result<(), String> {
        let mut inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_mut() {
            spatialization.set_direction(x, y, z);
            Ok(())
        } else {
            Err("Spatialization is not enabled".to_string())
        }
    }

    fn get_direction(&self) -> Result<(f32, f32, f32), String> {
        let inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_ref() {
            Ok(spatialization.get_direction())
        } else {
            Err("Spatialization is not enabled".to_string())
        }
    }

    fn set_velocity(&self, x: f32, y: f32, z: f32) -> Result<(), String> {
        let mut inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_mut() {
            spatialization.set_velocity(x, y, z);
            Ok(())
        } else {
            Err("Spatialization is not enabled".to_string())
        }
    }

    fn get_velocity(&self) -> Result<(f32, f32, f32), String> {
        let inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_ref() {
            Ok(spatialization.get_velocity())
        } else {
            Err("Spatialization is not enabled".to_string())
        }
    }

    fn set_speed_of_sound(&self, speed: f32) -> Result<(), String> {
        let mut inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_mut() {
            spatialization.set_speed_of_sound(speed);
            Ok(())
        } else {
            Err("Spatialization is not enabled".to_string())
        }
    }

    fn get_speed_of_sound(&self) -> Result<f32, String> {
        let inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_ref() {
            Ok(spatialization.get_speed_of_sound())
        } else {
            Err("Spatialization is not enabled".to_string())
        }
    }

    fn set_world_up(&self, x: f32, y: f32, z: f32) -> Result<(), String> {
        let mut inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_mut() {
            spatialization.set_world_up(x, y, z);
            Ok(())
        } else {
            Err("Spatialization is not enabled".to_string())
        }
    }

    fn get_world_up(&self) -> Result<(f32, f32, f32), String> {
        let inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_ref() {
            Ok(spatialization.get_world_up())
        } else {
            Err("Spatialization is not enabled".to_string())
        }
    }

    fn set_cone(&self, inner_angle: f32, outer_angle: f32, outer_gain: f32) -> Result<(), String> {
        let mut inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_mut() {
            spatialization.set_cone(inner_angle, outer_angle, outer_gain);
            Ok(())
        } else {
            Err("Spatialization is not enabled".to_string())
        }
    }

    fn get_cone(&self) -> Result<(f32, f32, f32), String> {
        let inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_ref() {
            Ok(spatialization.get_cone())
        } else {
            Err("Spatialization is not enabled".to_string())
        }
    }

    fn set_enabled(&self, is_enabled: bool) -> Result<(), String> {
        let mut inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_mut() {
            spatialization.set_enabled(is_enabled);
            Ok(())
        } else {
            Err("Spatialization is not enabled".to_string())
        }
    }

    fn is_enabled(&self) -> Result<bool, String> {
        let inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_ref() {
            Ok(spatialization.is_enabled())
        } else {
            Err("Spatialization is not enabled".to_string())
        }
    }
}

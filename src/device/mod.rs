use once_cell::sync::Lazy;
use std::sync::{Arc, Mutex, Weak, mpsc::Sender};
use thiserror::Error;

use inner::DeviceInner;

use crate::{
    context::{AudioHardwareInfo, DeviceType}, effects::{
        SpartialListenerHandler, SpatializationListener, SpatializationListenerError,
    }, math::Vector3, misc::{
        audioattributes::AudioAttributes,
        audiopropertyhandler::{PropertyError, PropertyHandler},
    }, mixer::inner::MixerChannel, sample::sampleinner::SampleChannelHandle as SampleChannel, track::inner::TrackChannel, utils
};

pub(crate) mod inner;

#[derive(Debug, Error)]
pub enum DeviceError {
    #[error("Audio device initialization failed with code: {}, {}", .0, self.ma_result_to_str())]
    InitializationError(i32),
    #[error("Invalid number of channels")]
    InvalidChannels,
    #[error("Invalid sample rate")]
    InvalidSampleRate,
    #[error("Invalid operation with code: {}, {}", .0, self.ma_result_to_str())]
    InvalidOperation(i32),
    #[error("Audio channel with reference id {0} not found")]
    ChannelNotFound(usize),
    #[error("Audio mixer with reference id {0} not found")]
    MixerNotFound(usize),
    #[error("Audio channel with reference id {0} already exists")]
    ChannelAlreadyExists(usize),
    #[error("Audio mixer with reference id {0} already exists")]
    MixerAlreadyExists(usize),
    #[error("Unsupported (or mismatched) selected hardware device")]
    UnsupportedHardwareDevice,
    #[error("Failed to send audio handle to audio thread")]
    SendAudioHandleFailed,
    #[error("{0}")]
    Other(Box<dyn std::error::Error + Send + 'static>), // Wraps other errors
}

impl DeviceError {
    pub fn from_other<E: std::error::Error + Send + 'static>(error: E) -> Self {
        DeviceError::Other(Box::new(error))
    }

    pub fn ma_result_to_str(&self) -> &str {
        match self {
            DeviceError::InitializationError(code)
            | DeviceError::InvalidOperation(code) => utils::ma_to_string_result(*code),
            _ => "N/A",
        }
    }
}

pub(crate) enum AudioHandle {
    Track(Weak<Mutex<TrackChannel>>),
    Sample(Weak<Mutex<SampleChannel>>),
    Mixer(Weak<Mutex<MixerChannel>>),
}

#[derive(Default, Debug, Clone)]
pub struct DeviceInfo<'a> {
    pub ty: DeviceType,
    pub channel: usize,
    pub sample_rate: f32,
    pub input: Option<&'a AudioHardwareInfo>,
    pub output: Option<&'a AudioHardwareInfo>,
}

/// A hardware audio device, used to play audio comes from Channel and Mixer.
pub struct Device {
    pub(crate) device_ref_id: u32,
    pub(crate) inner: Arc<Mutex<Box<DeviceInner>>>,
    pub(crate) sender: Sender<AudioHandle>,

    // Used for lifetime management of the hardware context
    #[allow(dead_code)]
    pub(crate) output: Option<AudioHardwareInfo>,
    #[allow(dead_code)]
    pub(crate) input: Option<AudioHardwareInfo>,
}

static DEVICE_ID_COUNTER: Lazy<Mutex<u32>> = Lazy::new(|| Mutex::new(0));

fn generate_device_id() -> u32 {
    let mut counter = DEVICE_ID_COUNTER.lock().unwrap();
    *counter += 1;
    *counter
}

impl Device {
    pub(crate) fn new(config: DeviceInfo) -> Result<Self, DeviceError> {
        let input = config.input.cloned();
        let output = config.output.cloned();

        let result = DeviceInner::new(config);
        if let Err(e) = result {
            return Err(e);
        }

        let (inner, sender) = result.unwrap();

        let new_id = generate_device_id();

        Ok(Device {
            device_ref_id: new_id,
            inner: Arc::new(Mutex::new(inner)),
            sender,
            input,
            output,
        })
    }

    pub fn start(&mut self) -> Result<(), DeviceError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(DeviceError::InvalidOperation(-1)); // Use a custom error code for lock failure
        };

        inner.start()
    }

    pub fn stop(&mut self) -> Result<(), DeviceError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(DeviceError::InvalidOperation(-1)); // Use a custom error code for lock failure
        };

        inner.stop()
    }

    /// Set callback for both input and output. If you want to set them separately, use set_input_callback and set_output_callback instead.
    pub fn set_callback<F>(&mut self, callback: Option<F>) -> Result<(), DeviceError>
    where
        F: FnMut(&[f32], &mut [f32]) + Send + 'static,
    {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(DeviceError::InvalidOperation(-1)); // Use a custom error code for lock failure
        };

        inner.set_callback(callback)
    }

    /// Set callback for input only. If you want to set both input and output callback at the same time, use set_callback instead.
    pub fn set_input_callback<F>(&mut self, callback: Option<F>) -> Result<(), DeviceError>
    where
        F: FnMut(&[f32]) + Send + 'static,
    {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(DeviceError::InvalidOperation(-1)); // Use a custom error code for lock failure
        };

        inner.set_input_callback(callback)
    }

    /// Set callback for output only. If you want to set both input and output callback at the same time, use set_callback instead.
    pub fn set_output_callback<F>(&mut self, callback: Option<F>) -> Result<(), DeviceError>
    where
        F: FnMut(&mut [f32]) + Send + 'static,
    {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(DeviceError::InvalidOperation(-1)); // Use a custom error code for lock failure
        };

        inner.set_output_callback(callback)
    }

    pub(crate) fn get_ref_id(&self) -> u32 {
        self.device_ref_id
    }

    pub(crate) fn attach_track(&mut self, track: &crate::Track) -> Result<(), DeviceError> {
        let weak = Arc::downgrade(&track.inner);

        if let Err(_) = self.sender.send(AudioHandle::Track(weak)) {
            return Err(DeviceError::SendAudioHandleFailed);
        }

        Ok(())
    }

    pub(crate) fn attach_sample(
        &mut self,
        sample: &crate::sample::SampleChannel,
    ) -> Result<(), DeviceError> {
        let weak = Arc::downgrade(&sample.inner);

        if let Err(_) = self.sender.send(AudioHandle::Sample(weak)) {
            return Err(DeviceError::SendAudioHandleFailed);
        }

        Ok(())
    }

    pub(crate) fn attach_mixer(&mut self, mixer: &crate::Mixer) -> Result<(), DeviceError> {
        let weak = Arc::downgrade(&mixer.inner);

        if let Err(_) = self.sender.send(AudioHandle::Mixer(weak)) {
            return Err(DeviceError::SendAudioHandleFailed);
        }

        Ok(())
    }
}

impl PropertyHandler for Device {
    fn get_attribute_f32(&self, _type: AudioAttributes) -> Result<f32, PropertyError> {
        let inner = self.inner.lock().unwrap();

        match _type {
            AudioAttributes::Unknown => {
                Err(PropertyError::UnsupportedAttribute("Unknown attribute"))
            }
            AudioAttributes::Volume => Ok(inner.volume.volume),
            AudioAttributes::Pan => Ok(inner.panner.pan),
            AudioAttributes::FXEnabled => Err(PropertyError::UnsupportedAttribute(
                "AudioFX is not supported, use set_attribute_bool to enable it",
            )),
            AudioAttributes::SpatializationEnabled => Err(PropertyError::UnsupportedAttribute(
                "AudioSpatialization is not supported, use set_attribute_bool to enable it",
            )),
            _ => Err(PropertyError::UnsupportedAttribute("Unsupported attribute")),
        }
    }

    fn set_attribute_f32(
        &mut self,
        _type: AudioAttributes,
        _value: f32,
    ) -> Result<(), PropertyError> {
        let mut inner = self.inner.lock().unwrap();

        match _type {
            AudioAttributes::Unknown => {
                Err(PropertyError::UnsupportedAttribute("Unknown attribute"))
            }
            AudioAttributes::Volume => {
                inner.volume.set_volume(_value);
                Ok(())
            }
            AudioAttributes::Pan => {
                inner.panner.set_pan(_value);
                Ok(())
            }
            AudioAttributes::FXEnabled => Err(PropertyError::UnsupportedAttribute(
                "AudioFX is not supported, use set_attribute_bool to enable it",
            )),
            AudioAttributes::SpatializationEnabled => Err(PropertyError::UnsupportedAttribute(
                "AudioSpatialization is not supported, use set_attribute_bool to enable it",
            )),
            _ => Err(PropertyError::UnsupportedAttribute("Unsupported attribute")),
        }
    }

    fn get_attribute_bool(&self, _type: AudioAttributes) -> Result<bool, PropertyError> {
        let inner = self.inner.lock().unwrap();

        match _type {
            AudioAttributes::Unknown => {
                Err(PropertyError::UnsupportedAttribute("Unknown attribute"))
            }
            AudioAttributes::SpatializationEnabled => Ok(inner.spatialization.is_some()),
            _ => Err(PropertyError::UnsupportedAttribute("Unsupported attribute")),
        }
    }

    fn set_attribute_bool(
        &mut self,
        _type: AudioAttributes,
        _value: bool,
    ) -> Result<(), PropertyError> {
        let mut inner = self.inner.lock().unwrap();

        match _type {
            AudioAttributes::Unknown => {
                Err(PropertyError::UnsupportedAttribute("Unknown attribute"))
            }
            AudioAttributes::SpatializationEnabled => {
                if _value {
                    let spatialization =
                        SpatializationListener::new(inner.device.playback.channels);
                    if let Err(e) = spatialization {
                        return Err(PropertyError::from_other(e));
                    }

                    inner.spatialization = spatialization.ok();
                } else {
                    inner.spatialization = None;
                }
                Ok(())
            }
            _ => Err(PropertyError::UnsupportedAttribute("Unsupported attribute")),
        }
    }
}

impl SpartialListenerHandler for Device {
    fn set_position(&self, position: Vector3<f32>) -> Result<(), SpatializationListenerError> {
        let mut inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_mut() {
            spatialization.set_position(position);
            Ok(())
        } else {
            Err(SpatializationListenerError::NotInitialized)
        }
    }

    fn get_position(&self) -> Result<Vector3<f32>, SpatializationListenerError> {
        let inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_ref() {
            Ok(spatialization.get_position())
        } else {
            Err(SpatializationListenerError::NotInitialized)
        }
    }

    fn set_direction(
        &self,
        position: Vector3<f32>,
    ) -> Result<(), SpatializationListenerError> {
        let mut inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_mut() {
            spatialization.set_direction(position);
            Ok(())
        } else {
            Err(SpatializationListenerError::NotInitialized)
        }
    }

    fn get_direction(&self) -> Result<Vector3<f32>, SpatializationListenerError> {
        let inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_ref() {
            Ok(spatialization.get_direction())
        } else {
            Err(SpatializationListenerError::NotInitialized)
        }
    }

    fn set_velocity(&self, position: Vector3<f32>) -> Result<(), SpatializationListenerError> {
        let mut inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_mut() {
            spatialization.set_velocity(position);
            Ok(())
        } else {
            Err(SpatializationListenerError::NotInitialized)
        }
    }

    fn get_velocity(&self) -> Result<Vector3<f32>, SpatializationListenerError> {
        let inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_ref() {
            Ok(spatialization.get_velocity())
        } else {
            Err(SpatializationListenerError::NotInitialized)
        }
    }

    fn set_speed_of_sound(&self, speed: f32) -> Result<(), SpatializationListenerError> {
        let mut inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_mut() {
            spatialization.set_speed_of_sound(speed);
            Ok(())
        } else {
            Err(SpatializationListenerError::NotInitialized)
        }
    }

    fn get_speed_of_sound(&self) -> Result<f32, SpatializationListenerError> {
        let inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_ref() {
            Ok(spatialization.get_speed_of_sound())
        } else {
            Err(SpatializationListenerError::NotInitialized)
        }
    }

    fn set_world_up(&self, position: Vector3<f32>) -> Result<(), SpatializationListenerError> {
        let mut inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_mut() {
            spatialization.set_world_up(position);
            Ok(())
        } else {
            Err(SpatializationListenerError::NotInitialized)
        }
    }

    fn get_world_up(&self) -> Result<Vector3<f32>, SpatializationListenerError> {
        let inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_ref() {
            Ok(spatialization.get_world_up())
        } else {
            Err(SpatializationListenerError::NotInitialized)
        }
    }

    fn set_cone(
        &self,
        inner_angle: f32,
        outer_angle: f32,
        outer_gain: f32,
    ) -> Result<(), SpatializationListenerError> {
        let mut inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_mut() {
            spatialization.set_cone(inner_angle, outer_angle, outer_gain);
            Ok(())
        } else {
            Err(SpatializationListenerError::NotInitialized)
        }
    }

    fn get_cone(&self) -> Result<(f32, f32, f32), SpatializationListenerError> {
        let inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_ref() {
            Ok(spatialization.get_cone())
        } else {
            Err(SpatializationListenerError::NotInitialized)
        }
    }

    fn set_enabled(&self, is_enabled: bool) -> Result<(), SpatializationListenerError> {
        let mut inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_mut() {
            spatialization.set_enabled(is_enabled);
            Ok(())
        } else {
            Err(SpatializationListenerError::NotInitialized)
        }
    }

    fn is_enabled(&self) -> Result<bool, SpatializationListenerError> {
        let inner_lock = self.inner.lock().unwrap();

        if let Some(spatialization) = inner_lock.spatialization.as_ref() {
            Ok(spatialization.is_enabled())
        } else {
            Err(SpatializationListenerError::NotInitialized)
        }
    }
}

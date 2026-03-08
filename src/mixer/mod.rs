use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use inner::MixerChannel;
use thiserror::Error;

use crate::{
    Device, effects::{AudioFX, AudioFXError}, misc::{
        audioattributes::AudioAttributes,
        audiopropertyhandler::{PropertyError, PropertyHandler},
    }, sample::SampleChannel, track::Track
};

pub(crate) mod inner;

static MIXER_ID: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Error)]
pub enum MixerError {
    #[error("Mixer already in use by another device with ref id: {0}")]
    InvalidDeviceRefId(u32),
    #[error("Invalid channel count: {0}")]
    InvalidChannelCount(usize),
    #[error("Invalid sample rate: {0}")]
    InvalidSampleRate(f32),
    #[error("Seek out of bounds: {0}")]
    IndexOutOfBounds(usize),
    #[error("Invalid operation: {0}")]
    InvalidOperation(&'static str),
    #[error("Failed to lock mixer")]
    LockFailed,
    #[error("{0}")]
    Other(Box<dyn std::error::Error + 'static>),
}

impl MixerError {
    pub fn from_other<E: std::error::Error + 'static>(error: E) -> Self {
        MixerError::Other(Box::new(error))
    }
}

#[derive(Debug)]
pub enum MixerInput<'a> {
    Track(&'a Track),
    Mixer(&'a Mixer),
    Sample(&'a SampleChannel),
}

#[derive(Debug, Default)]
pub struct MixerInfo<'a> {
    pub sample_rate: f32,
    pub channel: usize,
    pub tracks: Vec<MixerInput<'a>>,
}

#[derive(Debug)]
pub struct Mixer {
    pub(crate) device_ref_id: u32,
    pub(crate) inner: Arc<Mutex<MixerChannel>>,
    is_playing: Arc<AtomicBool>,
}

impl Mixer {
    pub fn new(info: MixerInfo) -> Result<Self, MixerError> {
        let inner = MixerChannel::new(
            info.channel,
            info.sample_rate,
            MIXER_ID.fetch_add(1, Ordering::SeqCst),
        )?;

        let is_playing = {
            let lock = inner.is_playing.clone();
            lock.store(false, Ordering::SeqCst);
            lock
        };

        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
            is_playing,
            device_ref_id: u32::MAX,
        })
    }

    pub fn play(&mut self, device: &mut Device) -> Result<(), MixerError> {
        let device_id = device.get_ref_id();
        if device_id != self.device_ref_id && self.device_ref_id != u32::MAX {
            return Err(MixerError::InvalidDeviceRefId(self.device_ref_id));
        }

        self.device_ref_id = device_id;

        if let Err(e) = device.attach_mixer(self) {
            return Err(MixerError::from_other(e));
        }

        let Ok(mut inner) = self.inner.lock() else {
            return Err(MixerError::LockFailed);
        };

        inner.start();
        inner.seek(Some(0))?;

        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), MixerError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(MixerError::LockFailed);
        };

        inner.stop();
        Ok(())
    }

    pub fn seek(&mut self, position: usize) -> Result<usize, MixerError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(MixerError::LockFailed);
        };

        inner.seek(Some(position))
    }

    pub fn set_normalize_output(&mut self, value: bool) -> Result<(), MixerError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(MixerError::LockFailed);
        };

        inner.set_normalize_output(value);
        Ok(())
    }

    pub fn set_callback<F>(&mut self, callback: F) -> Result<(), MixerError>
    where
        F: FnMut(&[f32]) + Send + 'static,
    {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(MixerError::LockFailed);
        };

        inner.dsp_callback = Some(Box::new(callback));
        Ok(())
    }

    pub fn add_track(&mut self, channel: &Track) -> Result<(), MixerError> {
        self.add_track_ex(channel, None, None)
    }

    pub fn add_track_ex(
        &mut self,
        channel: &Track,
        delay: Option<usize>,
        duration: Option<usize>,
    ) -> Result<(), MixerError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(MixerError::LockFailed);
        };

        let channel_weak = Arc::downgrade(&channel.inner);
        inner.add_track(channel_weak, delay, duration)
    }

    pub fn remove_track(&mut self, track: &Track) -> Result<(), MixerError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(MixerError::LockFailed);
        };

        let track_weak = Arc::downgrade(&track.inner);
        inner.remove_track(&track_weak)
    }

    pub fn add_mixer(&mut self, mixer: &Mixer) -> Result<(), MixerError> {
        self.add_mixer_ex(mixer, None, None)
    }

    pub fn add_mixer_ex(
        &mut self,
        mixer: &Mixer,
        delay: Option<usize>,
        duration: Option<usize>,
    ) -> Result<(), MixerError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(MixerError::LockFailed);
        };

        let mixer_weak = Arc::downgrade(&mixer.inner);
        inner.add_mixer(mixer_weak, delay, duration)
    }

    pub fn remove_mixer(&mut self, mixer: &Mixer) -> Result<(), MixerError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(MixerError::LockFailed);
        };

        let mixer_weak = Arc::downgrade(&mixer.inner);
        inner.remove_mixer(&mixer_weak)
    }

    pub fn add_sample(&mut self, sample: &SampleChannel) -> Result<(), MixerError> {
        self.add_sample_ex(sample, None, None)
    }

    pub fn add_sample_ex(
        &mut self,
        sample: &SampleChannel,
        delay: Option<usize>,
        duration: Option<usize>,
    ) -> Result<(), MixerError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(MixerError::LockFailed);
        };

        let sample_weak = Arc::downgrade(&sample.inner);
        inner.add_sample(sample_weak, delay, duration)
    }

    pub fn remove_sample(&mut self, sample: &SampleChannel) -> Result<(), MixerError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(MixerError::LockFailed);
        };

        let sample_weak = Arc::downgrade(&sample.inner);
        inner.remove_sample(&sample_weak)
    }

    pub fn get_length(&self) -> Result<usize, MixerError> {
        let Ok(inner) = self.inner.lock() else {
            return Err(MixerError::LockFailed);
        };

        if inner.is_infinite {
            return Ok(usize::MAX);
        }

        Ok(inner.max_length)
    }

    pub fn get_position(&self) -> Result<usize, MixerError> {
        let Ok(inner) = self.inner.lock() else {
            return Err(MixerError::LockFailed);
        };

        Ok(inner.mixer_position)
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::SeqCst)
    }

    pub fn ref_id(&self) -> usize {
        let Ok(inner) = self.inner.lock() else {
            return usize::MAX;
        };

        inner.ref_id
    }
}

impl PropertyHandler for Mixer {
    fn get_attribute_f32(&self, _type: AudioAttributes) -> Result<f32, PropertyError> {
        let inner = self.inner.lock();
        if inner.is_err() {
            return Err(PropertyError::from_other(MixerError::InvalidOperation(
                "Failed to lock mixer state",
            )));
        }

        let inner = inner.unwrap();

        match _type {
            AudioAttributes::SampleRate => Ok(inner.resampler.sample_rate as f32),
            AudioAttributes::Volume => Ok(inner.volume.volume as f32),
            AudioAttributes::Pan => Ok(inner.panner.pan as f32),
            AudioAttributes::FXPitch => {
                if let Some(fx) = inner.fx.as_ref() {
                    Ok(fx.octave as f32)
                } else {
                    Err(PropertyError::Other(Box::new(AudioFXError::NotEnabled)))
                }
            }
            AudioAttributes::FXTempo => {
                if let Some(fx) = inner.fx.as_ref() {
                    Ok(fx.tempo as f32)
                } else {
                    Err(PropertyError::Other(Box::new(AudioFXError::NotEnabled)))
                }
            }
            _ => Err(PropertyError::UnsupportedAttribute("Unknown attribute")),
        }
    }

    fn set_attribute_f32(
        &mut self,
        _type: AudioAttributes,
        _value: f32,
    ) -> Result<(), PropertyError> {
        let inner = self.inner.lock();
        if inner.is_err() {
            return Err(PropertyError::from_other(MixerError::InvalidOperation(
                "Failed to lock mixer state",
            )));
        }

        let mut inner = inner.unwrap();

        match _type {
            AudioAttributes::SampleRate => {
                inner.resampler.set_target_sample_rate(_value);
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
            AudioAttributes::FXPitch => {
                if let Some(fx) = inner.fx.as_mut() {
                    if let Err(e) = fx.set_octave(_value) {
                        return Err(PropertyError::from_other(e));
                    }

                    Ok(())
                } else {
                    Err(PropertyError::from_other(AudioFXError::NotEnabled))
                }
            }
            AudioAttributes::FXTempo => {
                if let Some(fx) = inner.fx.as_mut() {
                    if let Err(e) = fx.set_tempo(_value) {
                        return Err(PropertyError::from_other(e));
                    }

                    Ok(())
                } else {
                    Err(PropertyError::from_other(AudioFXError::NotEnabled))
                }
            }
            _ => Err(PropertyError::UnsupportedAttribute("Unknown attribute")),
        }
    }

    fn get_attribute_bool(&self, _type: AudioAttributes) -> Result<bool, PropertyError> {
        let inner = self.inner.lock();
        if inner.is_err() {
            return Err(PropertyError::from_other(MixerError::InvalidOperation(
                "Failed to lock mixer state",
            )));
        }

        let inner = inner.unwrap();

        match _type {
            AudioAttributes::FXEnabled => Ok(inner.fx.is_some()),
            AudioAttributes::SpatializationEnabled => {
                // TODO:
                Ok(false)
            }
            _ => Err(PropertyError::UnsupportedAttribute("Unknown attribute")),
        }
    }

    fn set_attribute_bool(
        &mut self,
        _type: AudioAttributes,
        _value: bool,
    ) -> Result<(), PropertyError> {
        let inner = self.inner.lock();
        if inner.is_err() {
            return Err(PropertyError::from_other(MixerError::InvalidOperation(
                "Failed to lock mixer state",
            )));
        }

        let mut inner = inner.unwrap();

        match _type {
            AudioAttributes::FXEnabled => {
                if _value {
                    let fx = AudioFX::new(inner.channel_count, inner.resampler.sample_rate)
                        .map_err(PropertyError::from_other)?;

                    inner.fx = Some(fx);
                } else {
                    inner.fx = None;
                }

                let seek_pos = inner.mixer_position;
                _ = inner.seek(Some(seek_pos));

                Ok(())
            }
            AudioAttributes::SpatializationEnabled => {
                // TODO
                Ok(())
            }
            _ => Err(PropertyError::UnsupportedAttribute("Unknown attribute")),
        }
    }
}

impl Drop for Mixer {
    fn drop(&mut self) {
        let inner = self.inner.lock();
        if inner.is_err() {
            return;
        }

        let mut inner = inner.unwrap();

        inner.is_playing.store(false, Ordering::SeqCst);
        inner.marked_as_deleted = true;
    }
}

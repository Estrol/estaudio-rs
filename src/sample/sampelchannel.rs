use std::sync::{Arc, Mutex, atomic::Ordering};

use crate::{
    audioreader::cache::AudioCache, device::Device, effects::AudioFX, misc::{
        audioattributes::AudioAttributes,
        audiopropertyhandler::{PropertyError, PropertyHandler},
    }, sample::sampleinner::{AtomicSampleChannelStatus, SampleChannelError}
};

use super::{SampleChannelStatus, SampleError, sampleinner::SampleChannelHandle};

#[derive(Debug, Clone)]
pub struct SampleChannel {
    pub(crate) device_ref_id: u32,
    pub(crate) status: Arc<AtomicSampleChannelStatus>,
    pub(crate) inner: Arc<Mutex<SampleChannelHandle>>,
}

impl SampleChannel {
    pub(crate) fn get_inner_counter(&self) -> usize {
        Arc::strong_count(&self.inner)
    }

    pub(crate) fn new(
        cache: &Option<Arc<AudioCache>>,
        buffer: &Option<crate::BufferInfo>,
        channel: usize,
        sample_rate: f32,
    ) -> Result<Self, SampleChannelError> {
        let inner = SampleChannelHandle::new(cache, buffer, channel, sample_rate)?;

        let status = Arc::clone(&inner.status);

        Ok(Self {
            device_ref_id: u32::MAX,
            status,
            inner: Arc::new(Mutex::new(inner)),
        })
    }

    pub fn play(&mut self, device: &mut Device) -> Result<(), SampleChannelError> {
        let device_ref_id = device.get_ref_id();
        if device_ref_id != self.device_ref_id && self.device_ref_id != u32::MAX {
            return Err(SampleChannelError::InvalidDeviceRefId(self.device_ref_id));
        }

        self.device_ref_id = device_ref_id;

        if let Err(e) = device.attach_sample(self) {
            return Err(SampleChannelError::from_other(e));
        }

        self.status.store(SampleChannelStatus::Playing, Ordering::Relaxed);

        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), SampleError> {
        let Ok(handle) = self.inner.lock() else {
            return Err(SampleError::LockFailed);
        };

        handle
            .status
            .store(SampleChannelStatus::Finished, Ordering::Relaxed);

        self.device_ref_id = u32::MAX;

        Ok(())
    }

    pub fn is_finished(&self) -> bool {
        self.status.load(Ordering::Relaxed) == SampleChannelStatus::Finished
    }

    pub(crate) fn reset(&mut self, info: &Option<super::SampleChannelInfo>) {
        if let Ok(mut handle) = self.inner.lock() {
            handle
                .status
                .store(SampleChannelStatus::NotStarted, Ordering::Relaxed);

            if let Some(info) = info {
                if let Some(sample_rate) = info.sample_rate {
                    let _ = handle.resampler.set_target_sample_rate(sample_rate);
                }

                if let Some(channels) = info.channels {
                    let _ = handle
                        .channel_converter
                        .set_output_channels(channels as usize);
                }
            }
        }
    }
}

impl PropertyHandler for SampleChannel {
    fn get_attribute_f32(&self, _type: AudioAttributes) -> Result<f32, PropertyError> {
        let lock = crate::macros::check!(
            self.inner.lock(),
            PropertyError::InvalidOperation("Failed to lock SampleChannelHandle")
        );

        match _type {
            AudioAttributes::SampleRate => Ok(lock.resampler.sample_rate as f32),
            AudioAttributes::Volume => Ok(lock.volume.volume),
            AudioAttributes::Pan => Ok(lock.panner.pan),
            AudioAttributes::FXPitch => {
                if let Some(fx) = &lock.fx {
                    Ok(fx.octave)
                } else {
                    Err(PropertyError::InvalidOperation(
                        "FX must be enabled to get FXPitch",
                    ))
                }
            }
            AudioAttributes::FXTempo => {
                if let Some(fx) = &lock.fx {
                    Ok(fx.tempo)
                } else {
                    Err(PropertyError::InvalidOperation(
                        "FX must be enabled to get FXTempo",
                    ))
                }
            }
            _ => Err(PropertyError::UnsupportedAttribute("Unknown attribute")),
        }
    }

    fn set_attribute_f32(
        &mut self,
        _type: AudioAttributes,
        value: f32,
    ) -> Result<(), PropertyError> {
        let mut lock = crate::macros::check!(
            self.inner.lock(),
            PropertyError::InvalidOperation("Failed to lock SampleChannelHandle")
        );

        match _type {
            AudioAttributes::SampleRate => {
                lock.resampler.set_target_sample_rate(value);

                Ok(())
            }
            AudioAttributes::Volume => {
                lock.volume.set_volume(value);

                Ok(())
            }
            AudioAttributes::Pan => {
                lock.panner.set_pan(value);

                Ok(())
            }
            AudioAttributes::FXTempo => {
                if let Some(fx) = &mut lock.fx {
                    fx.set_tempo(value).map_err(PropertyError::from_other)
                } else {
                    Err(PropertyError::InvalidOperation(
                        "FX must be enabled to set FXTempo",
                    ))
                }
            }
            AudioAttributes::FXPitch => {
                if let Some(fx) = &mut lock.fx {
                    fx.set_octave(value).map_err(PropertyError::from_other)
                } else {
                    Err(PropertyError::InvalidOperation(
                        "FX must be enabled to set FXPitch",
                    ))
                }
            }
            _ => Err(PropertyError::UnsupportedAttribute("Unknown attribute")),
        }
    }

    fn get_attribute_bool(&self, _type: AudioAttributes) -> Result<bool, PropertyError> {
        let lock = crate::macros::check!(
            self.inner.lock(),
            PropertyError::InvalidOperation("Failed to lock SampleChannelHandle")
        );

        match _type {
            AudioAttributes::FXEnabled => Ok(lock.fx.is_some()),
            _ => Err(PropertyError::UnsupportedAttribute("Unknown attribute")),
        }
    }

    fn set_attribute_bool(
        &mut self,
        _type: AudioAttributes,
        value: bool,
    ) -> Result<(), PropertyError> {
        let mut lock = crate::macros::check!(
            self.inner.lock(),
            PropertyError::InvalidOperation("Failed to lock SampleChannelHandle")
        );

        match _type {
            AudioAttributes::FXEnabled => {
                if value && lock.fx.is_none() {
                    let sample_rate = lock.reader.sample_rate;
                    let channels = lock.reader.channels;

                    let fx = AudioFX::new(channels, sample_rate);
                    lock.fx = fx.ok();
                } else if !value {
                    lock.fx = None;
                }

                Ok(())
            }
            _ => Err(PropertyError::UnsupportedAttribute("Unknown attribute")),
        }
    }
}

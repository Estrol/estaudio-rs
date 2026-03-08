use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use inner::TrackChannel;
use thiserror::Error;

use crate::{
    device::Device, effects::{
        AttenuationModel, AudioFX, AudioFXError, Spatialization, SpatializationError,
        SpatializationHandler, Positioning,
    }, math::Vector3, misc::{
        audioattributes::AudioAttributes,
        audiopropertyhandler::{PropertyError, PropertyHandler},
    }, utils::TweenType
};

pub(crate) mod inner;

#[derive(Debug, Error)]
pub enum TrackError {
    #[error("Failed to create the track instance!")]
    CreateFailed,
    #[error("Failed to read from the track channel")]
    ReadError,
    #[error("Seek position is out of bounds")]
    SeekOutOfBounds,
    #[error("Failed to seek in the track channel")]
    SeekFailed,
    #[error("The track channel is attached to a different device")]
    InvalidDeviceId,
    #[error("Audio channel processing failed")]
    ProcessingFailed,
    #[error("Failed to lock the track channel")]
    LockFailed,
    #[error("{0}")]
    Other(Box<dyn std::error::Error + Send + 'static>),
}

impl TrackError {
    pub fn from_other<E: std::error::Error + Send + 'static>(error: E) -> Self {
        TrackError::Other(Box::new(error))
    }
}

#[allow(dead_code)]
pub(crate) struct TrackSliderInstance {
    pub start: f32,
    pub end: f32,
    pub tween: TweenType,
    pub current: f32,
}

static TRACK_ID: AtomicUsize = AtomicUsize::new(0);
static INVALID_DEVICE_REF_ID: u32 = u32::MAX;

#[derive(Debug, Default)]
pub struct TrackInfo<'a> {
    pub source: crate::Source<'a>,
    pub sample_rate: Option<f32>,
    pub channel: Option<usize>,
}

/// Represents an audio track that can play audio data, apply effects, and be spatialized.
#[derive(Debug, Clone)]
pub struct Track {
    pub(crate) ref_id: usize,
    pub(crate) inner: Arc<Mutex<TrackChannel>>,

    playing: Arc<AtomicBool>,
    is_looping: Arc<AtomicBool>,
    position: Arc<AtomicUsize>,
    sample_rate: f32,
    pcm_length: usize,
    device_ref_id: u32,
}

impl Track {
    pub(crate) fn new(info: TrackInfo) -> Result<Self, TrackError> {
        let (cache, buffer_info) = info.source.into_buffer();
        let id = TRACK_ID.fetch_add(1, Ordering::SeqCst);

        let Ok(track) =
            TrackChannel::new(id, cache, buffer_info, info.sample_rate, info.channel, true)
        else {
            return Err(TrackError::CreateFailed);
        };

        let pcm_length = track.reader.pcm_length;
        let sample_rate = track.resampler.target_sample_rate;
        let playing = Arc::clone(&track.playing);
        let position = Arc::clone(&track.position);
        let is_looping = Arc::clone(&track.is_looping);
        let inner = Arc::new(Mutex::new(track));

        Ok(Self {
            ref_id: id,
            inner,
            playing,
            is_looping,
            position,
            sample_rate,
            pcm_length,
            device_ref_id: INVALID_DEVICE_REF_ID,
        })
    }

    /// Play the track on the given audio device.
    ///
    /// By default, the track is parentless and can be played on any device. Once played, it becomes attached to that device
    /// and cannot be played on another device until stopped.
    pub fn play(&mut self, device: &mut Device) -> Result<(), TrackError> {
        let device_ref_id = device.get_ref_id();
        if self.device_ref_id != INVALID_DEVICE_REF_ID && self.device_ref_id != device_ref_id {
            return Err(TrackError::InvalidDeviceId);
        }

        self.device_ref_id = device_ref_id;

        if let Err(e) = device.attach_track(self) {
            return Err(TrackError::from_other(e));
        }

        let Ok(mut inner) = self.inner.lock() else {
            return Err(TrackError::SeekFailed);
        };

        inner.playing.store(true, Ordering::Release);
        inner.seek(0)?;

        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), TrackError> {
        let Some(inner) = self.inner.lock().ok() else {
            return Err(TrackError::LockFailed);
        };

        inner.playing.store(false, Ordering::Release);
        self.device_ref_id = INVALID_DEVICE_REF_ID;

        Ok(())
    }

    pub fn set_callback<F>(&mut self, callback: F) -> Result<(), TrackError>
    where
        F: FnMut(&mut [f32]) + Send + 'static,
    {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(TrackError::LockFailed);
        };

        inner.set_callback(callback);
        Ok(())
    }

    pub fn set_start(&mut self, start: Option<usize>) -> Result<(), TrackError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(TrackError::LockFailed);
        };

        inner.start = start;
        Ok(())
    }

    pub fn set_end(&mut self, end: Option<usize>) -> Result<(), TrackError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(TrackError::LockFailed);
        };

        inner.end = end;
        Ok(())
    }

    pub fn seek(&mut self, position: usize) -> Result<(), TrackError> {
        if position >= self.pcm_length {
            return Err(TrackError::SeekOutOfBounds);
        }

        let Ok(mut inner) = self.inner.lock() else {
            return Err(TrackError::LockFailed);
        };

        inner.seek(position)?;

        Ok(())
    }

    pub fn seek_ms(&mut self, position: usize) -> Result<(), TrackError> {
        let position = (position as f32 * self.sample_rate) / 1000.0;
        self.seek(position as usize)
    }

    pub fn is_playing(&self) -> bool {
        self.playing.load(Ordering::SeqCst)
    }

    pub fn get_position(&self) -> usize {
        self.position.load(Ordering::SeqCst)
    }

    pub fn set_looping(&mut self, looping: bool) {
        self.is_looping.store(looping, Ordering::SeqCst);
    }

    pub fn is_looping(&self) -> bool {
        self.is_looping.load(Ordering::SeqCst)
    }

    pub fn get_length(&self) -> usize {
        self.pcm_length
    }

    pub fn ref_id(&self) -> usize {
        self.ref_id
    }
}

impl PropertyHandler for Track {
    fn get_attribute_f32(&self, _type: AudioAttributes) -> Result<f32, PropertyError> {
        let Ok(inner) = self.inner.lock() else {
            return Err(PropertyError::from_other(TrackError::LockFailed));
        };

        let result = match _type {
            AudioAttributes::FXTempo => {
                if inner.fx.is_none() {
                    return Err(PropertyError::from_other(AudioFXError::NotEnabled));
                }

                let fx = inner.fx.as_ref().unwrap();
                fx.tempo
            }
            AudioAttributes::FXPitch => {
                if inner.fx.is_none() {
                    return Err(PropertyError::from_other(AudioFXError::NotEnabled));
                }

                let fx = inner.fx.as_ref().unwrap();
                fx.octave
            }
            AudioAttributes::SampleRate => inner.resampler.target_sample_rate as f32,
            AudioAttributes::Volume => inner.gainer.volume,
            AudioAttributes::Pan => inner.panner.pan,
            _ => {
                return Err(PropertyError::UnsupportedAttribute("Unsupported attribute"));
            }
        };

        Ok(result)
    }

    fn set_attribute_f32(
        &mut self,
        _type: AudioAttributes,
        _value: f32,
    ) -> Result<(), PropertyError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(PropertyError::from_other(TrackError::LockFailed));
        };

        match _type {
            AudioAttributes::FXTempo => {
                if inner.fx.is_none() {
                    return Err(PropertyError::from_other(AudioFXError::NotEnabled));
                }

                let fx = inner.fx.as_mut().unwrap();
                fx.set_tempo(_value).unwrap();
            }
            AudioAttributes::FXPitch => {
                if inner.fx.is_none() {
                    return Err(PropertyError::from_other(AudioFXError::NotEnabled));
                }

                let fx = inner.fx.as_mut().unwrap();
                fx.set_octave(_value).unwrap();
            }
            AudioAttributes::SampleRate => {
                inner.resampler.set_target_sample_rate(_value);
            }
            AudioAttributes::Volume => {
                inner.gainer.set_volume(_value);
            }
            AudioAttributes::Pan => {
                inner.panner.set_pan(_value);
            }
            _ => {
                return Err(PropertyError::UnsupportedAttribute("Unknown attribute"));
            }
        };

        Ok(())
    }

    fn get_attribute_bool(&self, _type: AudioAttributes) -> Result<bool, PropertyError> {
        let Ok(inner) = self.inner.lock() else {
            return Err(PropertyError::from_other(TrackError::LockFailed));
        };

        match _type {
            AudioAttributes::FXEnabled => Ok(inner.fx.is_some()),
            AudioAttributes::SpatializationEnabled => Ok(inner.spatializer.is_some()),
            _ => Err(PropertyError::UnsupportedAttribute("Unsupported attribute")),
        }
    }

    fn set_attribute_bool(
        &mut self,
        _type: AudioAttributes,
        _value: bool,
    ) -> Result<(), PropertyError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(PropertyError::from_other(TrackError::LockFailed));
        };

        match _type {
            AudioAttributes::FXEnabled => {
                if _value {
                    if inner.fx.is_none() {
                        let fx = AudioFX::new(inner.reader.channels, inner.reader.sample_rate);

                        if let Err(e) = fx {
                            return Err(PropertyError::from_other(e));
                        }

                        inner.fx = fx.ok();
                    }
                } else {
                    inner.fx = None;
                }

                let seek_pos = inner.position.load(Ordering::SeqCst);
                let seek_result = inner.seek(seek_pos);

                if let Err(e) = seek_result {
                    return Err(PropertyError::from_other(e));
                }
            }
            AudioAttributes::SpatializationEnabled => {
                if _value {
                    if inner.spatializer.is_none() {
                        let spatializer =
                            Spatialization::new(inner.reader.channels, inner.reader.channels);

                        if let Err(e) = spatializer {
                            return Err(PropertyError::from_other(e));
                        }

                        inner.spatializer = spatializer.ok();
                    }
                } else {
                    inner.spatializer = None;
                }
            }
            _ => {
                return Err(PropertyError::UnsupportedAttribute("Unsupported attribute"));
            }
        }

        Ok(())
    }
}

impl SpatializationHandler for Track {
    fn spatial_set_position(&mut self, position: Vector3<f32>) -> Result<(), SpatializationError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(SpatializationError::from_other(TrackError::LockFailed));
        };

        let Some(spatializer) = inner.spatializer.as_mut() else {
            return Err(SpatializationError::NotInitialized);
        };
        
        spatializer.set_position(position);
        Ok(())
    }

    fn spatial_get_position(&self) -> Result<Vector3<f32>, SpatializationError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(SpatializationError::from_other(TrackError::LockFailed));
        };

        let Some(spatializer) = inner.spatializer.as_mut() else {
            return Err(SpatializationError::NotInitialized);
        };

        Ok(spatializer.get_position())
    }

    fn spatial_set_velocity(&mut self, position: Vector3<f32>) -> Result<(), SpatializationError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(SpatializationError::from_other(TrackError::LockFailed));
        };

        let Some(spatializer) = inner.spatializer.as_mut() else {
            return Err(SpatializationError::NotInitialized);
        };

        spatializer.set_velocity(position);
        Ok(())
    }

    fn spatial_get_velocity(&self) -> Result<Vector3<f32>, SpatializationError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(SpatializationError::from_other(TrackError::LockFailed));
        };

        let Some(spatializer) = inner.spatializer.as_mut() else {
            return Err(SpatializationError::NotInitialized);
        };

        Ok(spatializer.get_velocity())
    }

    fn spatial_set_direction(&mut self, position: Vector3<f32>) -> Result<(), SpatializationError> {
       let Ok(mut inner) = self.inner.lock() else {
            return Err(SpatializationError::from_other(TrackError::LockFailed));
        };

        let Some(spatializer) = inner.spatializer.as_mut() else {
            return Err(SpatializationError::NotInitialized);
        };

        spatializer.set_direction(position);
        Ok(())
    }

    fn spatial_get_direction(&self) -> Result<Vector3<f32>, SpatializationError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(SpatializationError::from_other(TrackError::LockFailed));
        };

        let Some(spatializer) = inner.spatializer.as_mut() else {
            return Err(SpatializationError::NotInitialized);
        };

        Ok(spatializer.get_direction())
    }

    fn spatial_set_doppler_factor(&mut self, doppler_factor: f32) -> Result<(), SpatializationError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(SpatializationError::from_other(TrackError::LockFailed));
        };

        let Some(spatializer) = inner.spatializer.as_mut() else {
            return Err(SpatializationError::NotInitialized);
        };

        spatializer.set_doppler_factor(doppler_factor);
        Ok(())
    }

    fn spatial_get_doppler_factor(&self) -> Result<f32, SpatializationError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(SpatializationError::from_other(TrackError::LockFailed));
        };

        let Some(spatializer) = inner.spatializer.as_mut() else {
            return Err(SpatializationError::NotInitialized);
        };

        Ok(spatializer.get_doppler_factor())
    }

    fn spatial_set_attenuation_model(
        &mut self,
        attenuation_model: AttenuationModel,
    ) -> Result<(), SpatializationError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(SpatializationError::from_other(TrackError::LockFailed));
        };

        let Some(spatializer) = inner.spatializer.as_mut() else {
            return Err(SpatializationError::NotInitialized);
        };

        spatializer.set_attenuation_model(attenuation_model);
        Ok(())
    }

    fn spatial_get_attenuation_model(&self) -> Result<AttenuationModel, SpatializationError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(SpatializationError::from_other(TrackError::LockFailed));
        };

        let Some(spatializer) = inner.spatializer.as_mut() else {
            return Err(SpatializationError::NotInitialized);
        };

        Ok(spatializer.get_attenuation_model())
    }

    fn spatial_set_positioning(
        &mut self,
        positioning: Positioning,
    ) -> Result<(), SpatializationError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(SpatializationError::from_other(TrackError::LockFailed));
        };

        let Some(spatializer) = inner.spatializer.as_mut() else {
            return Err(SpatializationError::NotInitialized);
        };

        spatializer.set_positioning(positioning);
        Ok(())
    }

    fn spatial_get_positioning(&self) -> Result<Positioning, SpatializationError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(SpatializationError::from_other(TrackError::LockFailed));
        };

        let Some(spatializer) = inner.spatializer.as_mut() else {
            return Err(SpatializationError::NotInitialized);
        };

        Ok(spatializer.get_positioning())
    }

    fn spatial_set_rolloff(&mut self, rolloff: f32) -> Result<(), SpatializationError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(SpatializationError::from_other(TrackError::LockFailed));
        };

        let Some(spatializer) = inner.spatializer.as_mut() else {
            return Err(SpatializationError::NotInitialized);
        };

        spatializer.set_rolloff(rolloff);
        Ok(())
    }

    fn spatial_get_rolloff(&self) -> Result<f32, SpatializationError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(SpatializationError::from_other(TrackError::LockFailed));
        };

        let Some(spatializer) = inner.spatializer.as_mut() else {
            return Err(SpatializationError::NotInitialized);
        };

        Ok(spatializer.get_rolloff())
    }

    fn spatial_set_min_gain(&mut self, min_gain: f32) -> Result<(), SpatializationError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(SpatializationError::from_other(TrackError::LockFailed));
        };

        let Some(spatializer) = inner.spatializer.as_mut() else {
            return Err(SpatializationError::NotInitialized);
        };

        spatializer.set_min_gain(min_gain);
        Ok(())
    }

    fn spatial_get_min_gain(&self) -> Result<f32, SpatializationError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(SpatializationError::from_other(TrackError::LockFailed));
        };

        let Some(spatializer) = inner.spatializer.as_mut() else {
            return Err(SpatializationError::NotInitialized);
        };

        Ok(spatializer.get_min_gain())
    }

    fn spatial_set_max_gain(&mut self, max_gain: f32) -> Result<(), SpatializationError> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(spatializer) = inner.spatializer.as_mut() {
            spatializer.set_max_gain(max_gain);
            Ok(())
        } else {
            Err(SpatializationError::NotInitialized)
        }
    }

    fn spatial_get_max_gain(&self) -> Result<f32, SpatializationError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(SpatializationError::from_other(TrackError::LockFailed));
        };

        let Some(spatializer) = inner.spatializer.as_mut() else {
            return Err(SpatializationError::NotInitialized);
        };

        Ok(spatializer.get_max_gain())
    }

    fn spatial_set_min_distance(&mut self, min_distance: f32) -> Result<(), SpatializationError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(SpatializationError::from_other(TrackError::LockFailed));
        };

        let Some(spatializer) = inner.spatializer.as_mut() else {
            return Err(SpatializationError::NotInitialized);
        };

        spatializer.set_min_distance(min_distance);
        Ok(())
    }

    fn spatial_get_min_distance(&self) -> Result<f32, SpatializationError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(SpatializationError::from_other(TrackError::LockFailed));
        };

        let Some(spatializer) = inner.spatializer.as_mut() else {
            return Err(SpatializationError::NotInitialized);
        };

        Ok(spatializer.get_min_distance())
    }

    fn spatial_set_max_distance(&mut self, max_distance: f32) -> Result<(), SpatializationError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(SpatializationError::from_other(TrackError::LockFailed));
        };

        let Some(spatializer) = inner.spatializer.as_mut() else {
            return Err(SpatializationError::NotInitialized);
        };

        spatializer.set_max_distance(max_distance);
        Ok(())
    }

    fn spatial_get_max_distance(&self) -> Result<f32, SpatializationError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(SpatializationError::from_other(TrackError::LockFailed));
        };

        let Some(spatializer) = inner.spatializer.as_mut() else {
            return Err(SpatializationError::NotInitialized);
        };

        Ok(spatializer.get_max_distance())
    }

    fn spatial_set_cone(
        &mut self,
        inner_angle: f32,
        outer_angle: f32,
        outer_gain: f32,
    ) -> Result<(), SpatializationError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(SpatializationError::from_other(TrackError::LockFailed));
        };

        let Some(spatializer) = inner.spatializer.as_mut() else {
            return Err(SpatializationError::NotInitialized);
        };

        spatializer.set_cone(inner_angle, outer_angle, outer_gain);
        Ok(())
    }

    fn spatial_get_cone(&self) -> Result<(f32, f32, f32), SpatializationError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(SpatializationError::from_other(TrackError::LockFailed));
        };

        let Some(spatializer) = inner.spatializer.as_mut() else {
            return Err(SpatializationError::NotInitialized);
        };

        Ok(spatializer.get_cone())
    }

    fn spatial_set_directional_attenuation_factor(
        &mut self,
        directional_attenuation_factor: f32,
    ) -> Result<(), SpatializationError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(SpatializationError::from_other(TrackError::LockFailed));
        };

        let Some(spatializer) = inner.spatializer.as_mut() else {
            return Err(SpatializationError::NotInitialized);
        };

        spatializer.set_directional_attenuation_factor(directional_attenuation_factor);
        Ok(())
    }

    fn spatial_get_directional_attenuation_factor(&self) -> Result<f32, SpatializationError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(SpatializationError::from_other(TrackError::LockFailed));
        };

        let Some(spatializer) = inner.spatializer.as_mut() else {
            return Err(SpatializationError::NotInitialized);
        };

        Ok(spatializer.get_directional_attenuation_factor())
    }

    fn spatial_get_relative_position_and_direction(
        &self,
        listener: &Device,
    ) -> Result<(Vector3<f32>, Vector3<f32>), SpatializationError> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(SpatializationError::from_other(TrackError::LockFailed));
        };

        let Some(spatializer) = inner.spatializer.as_mut() else {
            return Err(SpatializationError::NotInitialized);
        };

        let Ok(mut listener_inner) = listener.inner.lock() else {
            return Err(SpatializationError::from_other(TrackError::LockFailed));
        };

        let Some(listener_spatializer) = listener_inner.spatialization.as_mut() else {
            return Err(SpatializationError::NotInitialized);
        };

        Ok(spatializer.get_relative_position_and_direction(listener_spatializer))
    }
}

impl Drop for Track {
    fn drop(&mut self) {
        let mut inner = self.inner.lock().unwrap();
        inner.marked_as_deleted = true;
        inner.playing.store(false, Ordering::SeqCst);
    }
}

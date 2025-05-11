use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    time::Instant,
};

use inner::AudioChannelInner;

use crate::{
    PCMIndex,
    device::{
        AudioAttributes, AudioDevice, AudioDeviceDSPCallback, AudioPropertyHandler,
        audioreader::AudioReader,
    },
    effects::{
        AttenuationModel, AudioFX, AudioPanner, AudioResampler, AudioSpatialization,
        AudioSpatializationHandler, AudioVolume, Positioning,
    },
    utils::{self, IntoOptionU64, MutexPoison, TweenType},
};

pub(crate) mod inner;

pub type AudioChannelDSPCallback = AudioDeviceDSPCallback;

pub trait AudioReaderHandler {
    /// Reads PCM frames from the audio channel and stores them in the output buffer.
    /// A temporary buffer is used for intermediate processing to improve performance.
    /// This method is suitable for real-time audio processing.
    fn read_pcm_frames(
        &mut self,
        output: &mut [f32],
        temp: &mut [f32],
        frame_count: u64,
    ) -> Result<u64, String>;

    /// Reads PCM frames from the audio channel and returns them as a vector.
    /// This method allocates memory for both the output and temporary buffers,
    /// making it less ideal for real-time audio processing.
    fn read_simple(&mut self, frame_count: u64) -> Result<Vec<f32>, String>;
}

pub trait AudioPropertySliderHandler {
    /// Set the attribute value (f32) of the audio channel.
    fn slide_attribute_f32(
        &mut self,
        _type: AudioAttributes,
        _start: f32,
        _end: f32,
        tween: TweenType,
    ) -> Result<(), String>;
}

pub(crate) struct AudioSliderInstance {
    pub start: f32,
    pub end: f32,
    pub tween: TweenType,
    pub current: f32,
}

static CHANNEL_ID: AtomicUsize = AtomicUsize::new(0);

pub struct AudioChannel {
    pub(crate) inner: Arc<Mutex<AudioChannelInner>>,

    playing: Arc<AtomicBool>,
    is_looping: Arc<AtomicBool>,
    position: Arc<AtomicU64>,
    pcm_length: u64,
    sample_rate: u32,
}

impl AudioChannel {
    fn create_inner(
        reader: AudioReader,
        sample_rate: u32,
    ) -> Result<
        (
            Arc<Mutex<AudioChannelInner>>,
            Arc<AtomicBool>,
            Arc<AtomicU64>,
            Arc<AtomicBool>,
        ),
        String,
    > {
        let atomic_playing = Arc::new(AtomicBool::new(false));
        let atomic_position = Arc::new(AtomicU64::new(0));
        let atomic_is_looping = Arc::new(AtomicBool::new(false));

        let panner = AudioPanner::new(reader.channels)?;
        let gainer = AudioVolume::new(reader.channels)?;
        let resampler = AudioResampler::new(reader.channels, sample_rate)?;
        let spatializer = AudioSpatialization::new(reader.channels, reader.channels)?;

        let inner = Arc::new(Mutex::new(AudioChannelInner {
            ref_id: CHANNEL_ID.fetch_add(1, Ordering::SeqCst),
            marked_as_deleted: false,
            reader,
            gainer,
            panner,
            resampler,
            playing: Arc::clone(&atomic_playing),
            position: Arc::clone(&atomic_position),
            is_looping: Arc::clone(&atomic_is_looping),
            fx: None,
            dsp_callback: None,
            spatializer: Some(spatializer),
            last_time: Instant::now(),
            start: None,
            end: None,
        }));

        Ok((inner, atomic_playing, atomic_position, atomic_is_looping))
    }

    pub(crate) fn new_file(file_path: &str) -> Result<Self, String> {
        let reader = AudioReader::load(file_path)?;
        let sample_rate = reader.sample_rate;
        let pcm_length = reader.pcm_length;

        let (inner, playing, position, is_looping) = Self::create_inner(reader, sample_rate)?;

        Ok(AudioChannel {
            inner,
            playing,
            position,
            is_looping,
            pcm_length,
            sample_rate,
        })
    }

    pub(crate) fn new_file_buffer(buffer: &[u8]) -> Result<Self, String> {
        let reader = AudioReader::load_file_buffer(buffer)?;
        let sample_rate = reader.sample_rate;
        let pcm_length = reader.pcm_length;

        let (inner, playing, position, is_looping) = Self::create_inner(reader, sample_rate)?;

        Ok(AudioChannel {
            inner,
            playing,
            position,
            is_looping,
            pcm_length,
            sample_rate,
        })
    }

    pub(crate) fn new_audio_buffer(
        data: &[f32],
        pcm_length: u64,
        sample_rate: u32,
        channels: u32,
    ) -> Result<Self, String> {
        let reader = AudioReader::load_audio_buffer(data, sample_rate, channels, pcm_length, true)?;

        let (inner, playing, position, is_looping) = Self::create_inner(reader, sample_rate)?;

        Ok(AudioChannel {
            inner,
            playing,
            position,
            is_looping,
            pcm_length,
            sample_rate,
        })
    }

    pub fn attach(&self, device: &AudioDevice) -> Result<(), String> {
        let inner_device = device.inner.lock_poison();
        let mut channels = inner_device.channels.lock_poison();

        channels.push(self.inner.clone());

        Ok(())
    }

    pub fn set_dsp_callback(&self, callback: Option<AudioChannelDSPCallback>) {
        let mut inner = self.inner.lock().unwrap();
        inner.dsp_callback = callback;
    }

    pub fn play(&self) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();
        inner.playing.store(true, Ordering::Release);

        if inner.position.load(Ordering::Acquire) == 0 {
            // Need to pre-buffer the fx if enabled
            inner.seek(0)?;
        }

        Ok(())
    }

    pub fn set_start(&self, start: Option<PCMIndex>) {
        let mut inner = self.inner.lock().unwrap();
        inner.start = start.into_option_u64();
    }

    pub fn set_end(&self, end: Option<PCMIndex>) {
        let mut inner = self.inner.lock().unwrap();
        inner.end = end.into_option_u64();
    }

    pub fn seek(&self, position: u64) -> Result<(), String> {
        if position >= self.pcm_length {
            return Err(format!("Position out of bounds: {}", position));
        }

        let mut inner = self.inner.lock().unwrap();
        inner.seek(position).map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn seek_ms(&self, position: u64) -> Result<(), String> {
        let position = (position * self.sample_rate as u64) / 1000;
        self.seek(position)
    }

    pub fn is_playing(&self) -> bool {
        self.playing.load(Ordering::SeqCst)
    }

    pub fn get_position(&self) -> u64 {
        self.position.load(Ordering::SeqCst)
    }

    pub fn set_looping(&self, looping: bool) {
        self.is_looping.store(looping, Ordering::SeqCst);
    }

    pub fn is_looping(&self) -> bool {
        self.is_looping.load(Ordering::SeqCst)
    }

    pub fn ref_id(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        inner.ref_id
    }
}

impl AudioReaderHandler for AudioChannel {
    fn read_pcm_frames(
        &mut self,
        output: &mut [f32],
        temp: &mut [f32],
        frame_count: u64,
    ) -> Result<u64, String> {
        let mut inner = self.inner.lock().unwrap();
        inner.read_pcm_frames(None, output, temp, frame_count)
    }

    fn read_simple(&mut self, frame_count: u64) -> Result<Vec<f32>, String> {
        if frame_count > 4096 {
            return Err("Frame count too large, use 4096 or lower".to_string());
        }

        let mut data = vec![0.0f32; 8192];
        let mut temp = vec![0.0f32; 8192];

        let mut inner = self.inner.lock().unwrap();
        let frames_readed = inner.read_pcm_frames(None, &mut data, &mut temp, frame_count)?;

        if frames_readed == 0 {
            return Ok(vec![]);
        }

        let mut output = vec![0.0f32; (frames_readed * inner.reader.channels as u64) as usize];
        utils::array_fast_copy_f32(
            &data,
            &mut output,
            0,
            0,
            (frames_readed * inner.reader.channels as u64) as usize,
        );

        Ok(output)
    }
}

impl AudioPropertyHandler for AudioChannel {
    fn get_attribute_f32(&self, _type: AudioAttributes) -> Result<f32, String> {
        let result = match _type {
            AudioAttributes::FXTempo => {
                let inner = self.inner.lock().unwrap();
                if inner.fx.is_none() {
                    return Err("FX not enabled".to_string());
                }

                let fx = inner.fx.as_ref().unwrap();
                fx.tempo
            }
            AudioAttributes::FXPitch => {
                let inner = self.inner.lock().unwrap();
                if inner.fx.is_none() {
                    return Err("FX not enabled".to_string());
                }

                let fx = inner.fx.as_ref().unwrap();
                fx.octave
            }
            AudioAttributes::SampleRate => {
                let inner = self.inner.lock().unwrap();
                inner.resampler.target_sample_rate as f32
            }
            AudioAttributes::Volume => {
                let inner = self.inner.lock().unwrap();
                inner.gainer.volume
            }
            AudioAttributes::Pan => {
                let inner = self.inner.lock().unwrap();
                inner.panner.pan
            }
            AudioAttributes::AudioFX => return Err("Unsupported attribute".to_string()),
            AudioAttributes::AudioSpatialization => return Err("Unsupported attribute".to_string()),
            AudioAttributes::Unknown => {
                return Err("Unknown attribute".to_string());
            }
        };

        Ok(result)
    }

    fn set_attribute_f32(&self, _type: AudioAttributes, _value: f32) -> Result<(), String> {
        match _type {
            AudioAttributes::FXTempo => {
                let mut inner = self.inner.lock().unwrap();
                if inner.fx.is_none() {
                    return Err("FX not enabled".to_string());
                }

                let fx = inner.fx.as_mut().unwrap();
                fx.set_tempo(_value).unwrap();
            }
            AudioAttributes::FXPitch => {
                let mut inner = self.inner.lock().unwrap();
                if inner.fx.is_none() {
                    return Err("FX not enabled".to_string());
                }

                let fx = inner.fx.as_mut().unwrap();
                fx.set_octave(_value).unwrap();
            }
            AudioAttributes::SampleRate => {
                let mut inner = self.inner.lock().unwrap();
                inner.resampler.set_target_sample_rate(_value as u32);
            }
            AudioAttributes::Volume => {
                let mut inner = self.inner.lock().unwrap();
                inner.gainer.set_volume(_value);
            }
            AudioAttributes::Pan => {
                let mut inner = self.inner.lock().unwrap();
                inner.panner.set_pan(_value);
            }
            AudioAttributes::AudioFX => {
                return Err("Unsupported attribute".to_string());
            }
            AudioAttributes::AudioSpatialization => {
                return Err("Unsupported attribute".to_string());
            }
            AudioAttributes::Unknown => {
                return Err("Unknown attribute".to_string());
            }
        };

        Ok(())
    }

    fn get_attribute_bool(&self, _type: AudioAttributes) -> Result<bool, String> {
        match _type {
            AudioAttributes::AudioFX => {
                let inner = self.inner.lock().unwrap();
                Ok(inner.fx.is_some())
            }
            AudioAttributes::AudioSpatialization => {
                let inner = self.inner.lock().unwrap();
                Ok(inner.spatializer.is_some())
            }
            _ => Err("Unsupported attribute".to_string()),
        }
    }

    fn set_attribute_bool(&self, _type: AudioAttributes, _value: bool) -> Result<(), String> {
        match _type {
            AudioAttributes::AudioFX => {
                let mut inner = self.inner.lock().unwrap();
                if _value {
                    if inner.fx.is_none() {
                        let fx = AudioFX::new(inner.reader.channels, inner.reader.sample_rate)?;
                        inner.fx = Some(fx);
                    }
                } else {
                    inner.fx = None;
                }

                let seek_pos = inner.position.load(Ordering::SeqCst);
                inner.seek(seek_pos)?;
            }
            AudioAttributes::AudioSpatialization => {
                let mut inner = self.inner.lock().unwrap();
                if _value {
                    if inner.spatializer.is_none() {
                        let spatializer = AudioSpatialization::new(
                            inner.reader.channels,
                            inner.reader.sample_rate,
                        )?;
                        inner.spatializer = Some(spatializer);
                    }
                } else {
                    inner.spatializer = None;
                }
            }
            _ => return Err("Unsupported attribute".to_string()),
        }

        Ok(())
    }
}

impl AudioSpatializationHandler for AudioChannel {
    fn set_position(&mut self, x: f32, y: f32, z: f32) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(spatializer) = inner.spatializer.as_mut() {
            spatializer.set_position(x, y, z);
            Ok(())
        } else {
            return Err("Spatialization not enabled".to_string());
        }
    }

    fn get_position(&self) -> Result<(f32, f32, f32), String> {
        let inner = self.inner.lock().unwrap();
        if let Some(spatializer) = inner.spatializer.as_ref() {
            Ok(spatializer.get_position())
        } else {
            return Err("Spatialization not enabled".to_string());
        }
    }

    fn set_velocity(&mut self, x: f32, y: f32, z: f32) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(spatializer) = inner.spatializer.as_mut() {
            spatializer.set_velocity(x, y, z);
            Ok(())
        } else {
            return Err("Spatialization not enabled".to_string());
        }
    }

    fn get_velocity(&self) -> Result<(f32, f32, f32), String> {
        let inner = self.inner.lock().unwrap();
        if let Some(spatializer) = inner.spatializer.as_ref() {
            Ok(spatializer.get_velocity())
        } else {
            return Err("Spatialization not enabled".to_string());
        }
    }

    fn set_direction(&mut self, x: f32, y: f32, z: f32) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(spatializer) = inner.spatializer.as_mut() {
            spatializer.set_direction(x, y, z);
            Ok(())
        } else {
            return Err("Spatialization not enabled".to_string());
        }
    }

    fn get_direction(&self) -> Result<(f32, f32, f32), String> {
        let inner = self.inner.lock().unwrap();
        if let Some(spatializer) = inner.spatializer.as_ref() {
            Ok(spatializer.get_direction())
        } else {
            return Err("Spatialization not enabled".to_string());
        }
    }

    fn set_doppler_factor(&mut self, doppler_factor: f32) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(spatializer) = inner.spatializer.as_mut() {
            spatializer.set_doppler_factor(doppler_factor);
            Ok(())
        } else {
            return Err("Spatialization not enabled".to_string());
        }
    }

    fn get_doppler_factor(&self) -> Result<f32, String> {
        let inner = self.inner.lock().unwrap();
        if let Some(spatializer) = inner.spatializer.as_ref() {
            Ok(spatializer.get_doppler_factor())
        } else {
            return Err("Spatialization not enabled".to_string());
        }
    }

    fn set_attenuation_model(&mut self, attenuation_model: AttenuationModel) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(spatializer) = inner.spatializer.as_mut() {
            spatializer.set_attenuation_model(attenuation_model);
            Ok(())
        } else {
            return Err("Spatialization not enabled".to_string());
        }
    }

    fn get_attenuation_model(&self) -> Result<AttenuationModel, String> {
        let inner = self.inner.lock().unwrap();
        if let Some(spatializer) = inner.spatializer.as_ref() {
            Ok(spatializer.get_attenuation_model())
        } else {
            return Err("Spatialization not enabled".to_string());
        }
    }

    fn set_positioning(&mut self, positioning: Positioning) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(spatializer) = inner.spatializer.as_mut() {
            spatializer.set_positioning(positioning);
            Ok(())
        } else {
            return Err("Spatialization not enabled".to_string());
        }
    }

    fn get_positioning(&self) -> Result<Positioning, String> {
        let inner = self.inner.lock().unwrap();
        if let Some(spatializer) = inner.spatializer.as_ref() {
            Ok(spatializer.get_positioning())
        } else {
            return Err("Spatialization not enabled".to_string());
        }
    }

    fn set_rolloff(&mut self, rolloff: f32) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(spatializer) = inner.spatializer.as_mut() {
            spatializer.set_rolloff(rolloff);
            Ok(())
        } else {
            return Err("Spatialization not enabled".to_string());
        }
    }

    fn get_rolloff(&self) -> Result<f32, String> {
        let inner = self.inner.lock().unwrap();
        if let Some(spatializer) = inner.spatializer.as_ref() {
            Ok(spatializer.get_rolloff())
        } else {
            return Err("Spatialization not enabled".to_string());
        }
    }

    fn set_min_gain(&mut self, min_gain: f32) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(spatializer) = inner.spatializer.as_mut() {
            spatializer.set_min_gain(min_gain);
            Ok(())
        } else {
            return Err("Spatialization not enabled".to_string());
        }
    }

    fn get_min_gain(&self) -> Result<f32, String> {
        let inner = self.inner.lock().unwrap();
        if let Some(spatializer) = inner.spatializer.as_ref() {
            Ok(spatializer.get_min_gain())
        } else {
            return Err("Spatialization not enabled".to_string());
        }
    }

    fn set_max_gain(&mut self, max_gain: f32) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(spatializer) = inner.spatializer.as_mut() {
            spatializer.set_max_gain(max_gain);
            Ok(())
        } else {
            return Err("Spatialization not enabled".to_string());
        }
    }

    fn get_max_gain(&self) -> Result<f32, String> {
        let inner = self.inner.lock().unwrap();
        if let Some(spatializer) = inner.spatializer.as_ref() {
            Ok(spatializer.get_max_gain())
        } else {
            return Err("Spatialization not enabled".to_string());
        }
    }

    fn set_min_distance(&mut self, min_distance: f32) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(spatializer) = inner.spatializer.as_mut() {
            spatializer.set_min_distance(min_distance);
            Ok(())
        } else {
            return Err("Spatialization not enabled".to_string());
        }
    }

    fn get_min_distance(&self) -> Result<f32, String> {
        let inner = self.inner.lock().unwrap();
        if let Some(spatializer) = inner.spatializer.as_ref() {
            Ok(spatializer.get_min_distance())
        } else {
            return Err("Spatialization not enabled".to_string());
        }
    }

    fn set_max_distance(&mut self, max_distance: f32) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(spatializer) = inner.spatializer.as_mut() {
            spatializer.set_max_distance(max_distance);
            Ok(())
        } else {
            return Err("Spatialization not enabled".to_string());
        }
    }

    fn get_max_distance(&self) -> Result<f32, String> {
        let inner = self.inner.lock().unwrap();
        if let Some(spatializer) = inner.spatializer.as_ref() {
            Ok(spatializer.get_max_distance())
        } else {
            return Err("Spatialization not enabled".to_string());
        }
    }

    fn set_cone(
        &mut self,
        inner_angle: f32,
        outer_angle: f32,
        outer_gain: f32,
    ) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(spatializer) = inner.spatializer.as_mut() {
            spatializer.set_cone(inner_angle, outer_angle, outer_gain);
            Ok(())
        } else {
            return Err("Spatialization not enabled".to_string());
        }
    }

    fn get_cone(&self) -> Result<(f32, f32, f32), String> {
        let inner = self.inner.lock().unwrap();
        if let Some(spatializer) = inner.spatializer.as_ref() {
            Ok(spatializer.get_cone())
        } else {
            return Err("Spatialization not enabled".to_string());
        }
    }

    fn set_directional_attenuation_factor(
        &mut self,
        directional_attenuation_factor: f32,
    ) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(spatializer) = inner.spatializer.as_mut() {
            spatializer.set_directional_attenuation_factor(directional_attenuation_factor);
            Ok(())
        } else {
            return Err("Spatialization not enabled".to_string());
        }
    }

    fn get_directional_attenuation_factor(&self) -> Result<f32, String> {
        let inner = self.inner.lock().unwrap();
        if let Some(spatializer) = inner.spatializer.as_ref() {
            Ok(spatializer.get_directional_attenuation_factor())
        } else {
            return Err("Spatialization not enabled".to_string());
        }
    }

    fn get_relative_position_and_direction(
        &self,
        listener: &AudioDevice,
    ) -> Result<((f32, f32, f32), (f32, f32, f32)), String> {
        let inner = self.inner.lock().unwrap();
        if let Some(spatializer) = inner.spatializer.as_ref() {
            let listener = listener.inner.lock_poison();

            if let Some(spatializer_listener) = listener.spatialization.as_ref() {
                Ok(spatializer.get_relative_position_and_direction(spatializer_listener))
            } else {
                return Err("Listener not enabled".to_string());
            }
        } else {
            return Err("Spatialization not enabled".to_string());
        }
    }
}

impl Drop for AudioChannel {
    fn drop(&mut self) {
        let mut inner = self.inner.lock().unwrap();
        inner.marked_as_deleted = true;
        inner.playing.store(false, Ordering::SeqCst);
    }
}

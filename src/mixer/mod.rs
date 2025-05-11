use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use inner::AudioMixerInner;

use crate::{
    channel::{AudioChannel, AudioReaderHandler}, device::{AudioAttributes, AudioDeviceDSPCallback, AudioPropertyHandler}, effects::AudioFX, utils::{IntoOptionU64, MutexPoison, PCMIndex}
};

pub(crate) mod inner;

static MIXER_ID: AtomicUsize = AtomicUsize::new(0);

pub type AudioMixerDSPCallback = AudioDeviceDSPCallback;

pub struct AudioMixer {
    pub(crate) inner: Arc<Mutex<AudioMixerInner>>,
    is_playing: Arc<AtomicBool>,
}

impl AudioMixer {
    pub fn new(channels: u32, sample_rate: u32) -> Result<Self, String> {
        let inner = AudioMixerInner::new(
            channels,
            sample_rate,
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
        })
    }

    pub fn play(&self) -> Result<(), String> {
        let mut inner = self.inner.lock_poison();
        Self::recursive_play(&mut inner, true)?;

        if inner.mixer_position == 0 {
            // Need pre-buffering the FX if audio fx is enabled
            inner.seek(Some(0))?;
        }

        Ok(())
    }

    pub fn stop(&self) -> Result<(), String> {
        let mut inner = self.inner.lock_poison();
        Self::recursive_play(&mut inner, false)
    }

    pub fn seek(&self, position: Option<PCMIndex>) -> Result<u64, String> {
        let mut inner = self.inner.lock_poison();
        inner.seek(position.into_option_u64())
    }

    pub fn set_dsp_callback(&self, callback: Option<AudioMixerDSPCallback>) -> Result<(), String> {
        let mut inner = self.inner.lock_poison();
        inner.dsp_callback = callback;
        Ok(())
    }

    fn recursive_play(inner: &mut AudioMixerInner, is_playing: bool) -> Result<(), String> {
        inner.is_playing.store(is_playing, Ordering::SeqCst);

        for channel in &inner.channels {
            let lock = channel.channel.lock_poison();
            lock.playing.store(is_playing, Ordering::SeqCst);
        }

        for mixer in &inner.mixers {
            let lock = mixer.mixer.lock_poison();
            let mut inner_mixer = lock;
            Self::recursive_play(&mut inner_mixer, is_playing)?;
        }

        Ok(())
    }

    pub fn add_channel(&self, channel: &AudioChannel) -> Result<(), String> {
        let mut inner = self.inner.lock_poison();
        inner.add_channel(channel.inner.clone(), None, None)?;
        Ok(())
    }

    pub fn add_channel_ex(
        &self,
        channel: &AudioChannel,
        delay: Option<PCMIndex>,
        duration: Option<PCMIndex>,
    ) -> Result<(), String> {
        let mut inner = self.inner.lock_poison();
        inner.add_channel(
            channel.inner.clone(),
            delay.into_option_u64(),
            duration.into_option_u64(),
        )?;
        Ok(())
    }

    pub fn remove_channel(&self, index: usize) -> Result<(), String> {
        let mut inner = self.inner.lock_poison();
        if index < inner.channels.len() {
            inner.channels.remove(index);
            Ok(())
        } else {
            Err("Index out of bounds".to_string())
        }
    }

    pub fn add_mixer(&self, mixer: &AudioMixer) -> Result<(), String> {
        let mut inner = self.inner.lock_poison();
        inner.add_mixer(mixer.inner.clone(), None, None)?;
        Ok(())
    }

    pub fn add_mixer_ex(
        &self,
        mixer: &AudioMixer,
        _delay: Option<PCMIndex>,
        _duration: Option<PCMIndex>,
    ) -> Result<(), String> {
        let mut inner = self.inner.lock_poison();
        inner.add_mixer(
            mixer.inner.clone(),
            _delay.into_option_u64(),
            _duration.into_option_u64(),
        )?;
        Ok(())
    }

    pub fn remove_mixer(&self, index: usize) -> Result<(), String> {
        let mut inner = self.inner.lock_poison();
        if index < inner.mixers.len() {
            inner.mixers.remove(index);
            Ok(())
        } else {
            Err("Index out of bounds".to_string())
        }
    }

    pub fn get_length(&self) -> Result<u64, String> {
        let inner = self.inner.lock_poison();

        if inner.is_infinite {
            return Ok(u64::MAX);
        }

        Ok(inner.max_length)
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::SeqCst)
    }

    pub fn ref_id(&self) -> usize {
        let inner = self.inner.lock_poison();
        inner.ref_id
    }
}

impl AudioReaderHandler for AudioMixer {
    fn read_pcm_frames(
        &mut self,
        output: &mut [f32],
        temp: &mut [f32],
        frame_count: u64,
    ) -> Result<u64, String> {
        if frame_count > 4096 {
            return Err("Frame count is too large".to_string());
        }

        let mut inner = self.inner.lock_poison();
        let readed = inner.read_pcm_frames(None, output, temp, frame_count)?;

        Ok(readed)
    }

    fn read_simple(&mut self, frame_count: u64) -> Result<Vec<f32>, String> {
        let mut inner = self.inner.lock_poison();

        let mut output = vec![0.0; frame_count as usize * inner.channel_count];
        let mut temp = vec![0.0; frame_count as usize * inner.channel_count];

        let readed = inner.read_pcm_frames(None, &mut output, &mut temp, frame_count)?;
        if readed == 0 {
            return Ok(vec![]);
        }

        Ok(output)
    }
}

impl AudioPropertyHandler for AudioMixer {
    fn get_attribute_f32(&self, _type: AudioAttributes) -> Result<f32, String> {
        let inner = self.inner.lock_poison();

        match _type {
            AudioAttributes::Unknown => Err("Unknown attribute".to_string()),
            AudioAttributes::SampleRate => {
                Ok(inner.resampler.sample_rate as f32)
            },
            AudioAttributes::Volume => {
                Ok(inner.volume.volume as f32)
            },
            AudioAttributes::Pan => {
                Ok(inner.panner.pan as f32)
            },
            AudioAttributes::FXPitch => {
                if let Some(fx) = inner.fx.as_ref() {
                    Ok(fx.octave as f32)
                } else {
                    Err("AudioFX is not enabled".to_string())
                }
            },
            AudioAttributes::FXTempo => {
                if let Some(fx) = inner.fx.as_ref() {
                    Ok(fx.tempo as f32)
                } else {
                    Err("AudioFX is not enabled".to_string())
                }
            },
            AudioAttributes::AudioFX => Err("Unsupported attribute".to_string()),
            AudioAttributes::AudioSpatialization => Err("Unsupported attribute".to_string()),
        }
    }

    fn set_attribute_f32(&self, _type: AudioAttributes, _value: f32) -> Result<(), String> {
        let mut inner = self.inner.lock_poison();

        match _type {
            AudioAttributes::Unknown => Err("Unknown attribute".to_string()),
            AudioAttributes::SampleRate => {
                inner.resampler.set_target_sample_rate(_value as u32);
                Ok(())
            },
            AudioAttributes::Volume => {
                inner.volume.set_volume(_value);
                Ok(())
            },
            AudioAttributes::Pan => {
                inner.panner.set_pan(_value);
                Ok(())
            },
            AudioAttributes::FXPitch => {
                if let Some(fx) = inner.fx.as_mut() {
                    fx.set_octave(_value)
                } else {
                    Err("AudioFX is not enabled".to_string())
                }
            },
            AudioAttributes::FXTempo => {
                if let Some(fx) = inner.fx.as_mut() {
                    fx.set_tempo(_value)
                } else {
                    Err("AudioFX is not enabled".to_string())
                }
            },
            AudioAttributes::AudioFX => Err("Unsupported attribute".to_string()),
            AudioAttributes::AudioSpatialization => Err("Unsupported attribute".to_string()),
        }
    }

    fn get_attribute_bool(&self, _type: AudioAttributes) -> Result<bool, String> {
        let inner = self.inner.lock_poison();

        match _type {
            AudioAttributes::Unknown => Err("Unknown attribute".to_string()),
            AudioAttributes::SampleRate => Err("Unsupported attribute".to_string()),
            AudioAttributes::Volume => Err("Unsupported attribute".to_string()),
            AudioAttributes::Pan => Err("Unsupported attribute".to_string()),
            AudioAttributes::FXPitch => Err("Unsupported attribute".to_string()),
            AudioAttributes::FXTempo => Err("Unsupported attribute".to_string()),
            AudioAttributes::AudioFX => {
                Ok(inner.fx.is_some())
            },
            AudioAttributes::AudioSpatialization => Err("Unsupported attribute".to_string()),
        }
    }

    fn set_attribute_bool(&self, _type: AudioAttributes, _value: bool) -> Result<(), String> {
        let mut inner = self.inner.lock_poison();

        match _type {
            AudioAttributes::Unknown => Err("Unknown attribute".to_string()),
            AudioAttributes::SampleRate => Err("Unsupported attribute".to_string()),
            AudioAttributes::Volume => Err("Unsupported attribute".to_string()),
            AudioAttributes::Pan => Err("Unsupported attribute".to_string()),
            AudioAttributes::FXPitch => Err("Unsupported attribute".to_string()),
            AudioAttributes::FXTempo => Err("Unsupported attribute".to_string()),
            AudioAttributes::AudioFX => {
                if _value {
                    inner.fx = Some(AudioFX::new(
                        inner.channel_count as u32,
                        inner.sample_rate,
                    )?);
                } else {
                    inner.fx = None;
                }

                let seek_pos = inner.mixer_position;
                inner.seek(Some(seek_pos))?;

                Ok(())
            },
            AudioAttributes::AudioSpatialization => Err("Unsupported attribute".to_string()),
        }
    }
}

impl Drop for AudioMixer {
    fn drop(&mut self) {
        let mut inner = self.inner.lock_poison();
        inner.is_playing.store(false, Ordering::SeqCst);
        inner.marked_as_deleted = true;
    }
}

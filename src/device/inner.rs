use miniaudio_sys::*;
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};

use crate::{
    channel::inner::AudioChannelInner,
    effects::{AudioFX, AudioPanner, AudioResampler, AudioSpatializationListener, AudioVolume},
    mixer::inner::AudioMixerInner,
    utils::{self, MutexPoison},
};

use super::{AudioDeviceDSPCallback, context::AudioHardwareInfo};

pub(crate) struct AudioDeviceInner {
    pub device: Box<ma_device>,
    pub channels: Arc<Mutex<Vec<Arc<Mutex<AudioChannelInner>>>>>,
    pub mixers: Arc<Mutex<Vec<Arc<Mutex<AudioMixerInner>>>>>,

    pub volume: AudioVolume,
    pub panner: AudioPanner,
    pub resampler: AudioResampler,
    pub fx: Option<AudioFX>,

    pub buffer: Vec<f32>,
    pub temp_buffer: Vec<f32>,

    pub resampler_buffer: Vec<f32>,

    // DSP callback
    pub dsp_callback: Option<AudioDeviceDSPCallback>,

    // Spatialization
    pub spatialization: Option<AudioSpatializationListener>,
}

impl<T> MutexPoison<T> for Mutex<T> {
    fn lock_poison(&self) -> MutexGuard<'_, T> {
        match self.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn try_lock_poison(&self) -> Option<MutexGuard<'_, T>> {
        match self.try_lock() {
            Ok(guard) => Some(guard),
            Err(TryLockError::Poisoned(poisoned)) => Some(poisoned.into_inner()),
            Err(TryLockError::WouldBlock) => None,
        }
    }
}

impl AudioDeviceInner {
    pub fn new(
        info: Option<&AudioHardwareInfo>,
        channels: u32,
        sample_rate: u32,
    ) -> Result<Box<AudioDeviceInner>, String> {
        unsafe {
            let mut inner = Box::new(AudioDeviceInner {
                device: Box::new(std::mem::zeroed()),
                channels: Arc::new(Mutex::new(vec![])),
                mixers: Arc::new(Mutex::new(vec![])),
                buffer: vec![0.0f32; 4096 * channels as usize],
                temp_buffer: vec![0.0f32; 4096 * channels as usize],
                resampler_buffer: vec![0.0f32; 4096 * channels as usize],
                spatialization: None,
                volume: AudioVolume::new(channels)?,
                panner: AudioPanner::new(channels)?,
                resampler: AudioResampler::new(channels, sample_rate)?,
                dsp_callback: None,
                fx: None,
            });

            let mut config = ma_device_config_init(ma_device_type_playback);

            config.playback.format = ma_format_f32;
            config.playback.channels = channels;
            config.sampleRate = sample_rate;
            config.dataCallback = Some(audio_callback);
            config.pUserData = inner.as_mut() as *mut _ as *mut std::ffi::c_void;

            let mut context = None;
            if let Some(hw_info) = info {
                config.playback.pDeviceID = &hw_info.id;
                context = Some(hw_info.context.clone());
            }

            let result = if let Some(context) = context {
                let context_lock = context.lock_poison();
                let mut ma_device_lock = context_lock.context.lock_poison();
                ma_device_init(ma_device_lock.as_mut(), &config, inner.device.as_mut())
            } else {
                ma_device_init(std::ptr::null_mut(), &config, inner.device.as_mut())
            };

            if result != MA_SUCCESS {
                return Err(format!("Failed to initialize device: {}", result));
            }

            let result = ma_device_start(inner.device.as_mut());
            if result != MA_SUCCESS {
                return Err(format!("Failed to start device: {}", result));
            }

            Ok(inner)
        }
    }

    pub fn process(&mut self, output: &mut [f32], frame_count: u64) -> Result<(), String> {
        utils::array_fast_set_value_f32(output, 0.0);

        let mut channels = self.channels.lock_poison();
        let mut mixers = self.mixers.lock_poison();

        if channels.is_empty() && mixers.is_empty() {
            return Ok(());
        }

        let required_frame_count = self.resampler.get_required_input(frame_count);

        if required_frame_count.is_err() {
            return Err(format!(
                "Error getting required frame count: {}",
                required_frame_count.err().unwrap()
            ));
        }

        let required_frame_count = required_frame_count.unwrap();
        let channel_count = self.device.playback.channels as usize;

        utils::array_fast_set_value_f32(&mut self.resampler_buffer, 0.0);

        if self.fx.is_some() {
            let fx = self.fx.as_mut().unwrap();

            let mut target_frame_count = required_frame_count;
            let readed_frame_count = required_frame_count;

            if !fx.tempo_bypass() {
                let required_frame_count = fx.get_required_input(frame_count);
                if required_frame_count.is_err() {
                    return Err(format!(
                        "Error getting required frame count: {}",
                        required_frame_count.err().unwrap()
                    ));
                }

                target_frame_count = required_frame_count.unwrap();
            }

            utils::array_fast_set_value_f32(&mut self.buffer, 0.0);
            utils::array_fast_set_value_f32(&mut self.temp_buffer, 0.0);

            let mut max_frames_readed = 0;
            for channel in channels.iter_mut() {
                if let Some(mut lock) = channel.try_lock_poison() {
                    let frames_read = lock
                        .read_pcm_frames(
                            self.spatialization.as_mut(),
                            &mut self.buffer,
                            &mut self.temp_buffer,
                            target_frame_count,
                        )
                        .unwrap_or(0);

                    if frames_read > 0 {
                        utils::array_fast_add_value_f32(
                            &self.buffer,
                            &mut self.resampler_buffer,
                            (frames_read as usize * channel_count) as usize,
                        );
                    }

                    max_frames_readed = max_frames_readed.max(frames_read);
                }
            }

            utils::array_fast_set_value_f32(&mut self.buffer, 0.0);
            utils::array_fast_set_value_f32(&mut self.temp_buffer, 0.0);

            for mixer in mixers.iter_mut() {
                if let Some(mut lock) = mixer.try_lock_poison() {
                    let frames_read = lock
                        .read_pcm_frames(
                            self.spatialization.as_mut(),
                            &mut self.buffer,
                            &mut self.temp_buffer,
                            target_frame_count,
                        )
                        .unwrap_or(0);

                    if frames_read > 0 {
                        utils::array_fast_add_value_f32(
                            &self.buffer,
                            &mut self.resampler_buffer,
                            (frames_read as usize * channel_count) as usize,
                        );
                    }

                    max_frames_readed = max_frames_readed.max(frames_read);
                }
            }

            fx.frame_available += max_frames_readed as i64;

            if fx.frame_available > 0 {
                let readed = fx.process(
                    &self.resampler_buffer,
                    target_frame_count,
                    &mut self.buffer,
                    readed_frame_count,
                );

                if readed.is_err() {
                    return Err(format!(
                        "Error processing audio FX: {}",
                        readed.err().unwrap()
                    ));
                }

                fx.frame_available -= readed_frame_count as i64;

                if fx.frame_available < 0 {
                    fx.frame_available = 0;
                }
            }

            utils::array_fast_copy_f32(
                &self.buffer,
                &mut self.resampler_buffer,
                0,
                0,
                (required_frame_count as usize * channel_count) as usize,
            );
        } else {
            utils::array_fast_set_value_f32(&mut self.buffer, 0.0);
            utils::array_fast_set_value_f32(&mut self.temp_buffer, 0.0);

            for channel in channels.iter_mut() {
                if let Some(mut lock) = channel.try_lock_poison() {
                    let frames_read = lock
                        .read_pcm_frames(
                            self.spatialization.as_mut(),
                            &mut self.buffer,
                            &mut self.temp_buffer,
                            required_frame_count,
                        )
                        .unwrap_or(0);

                    if frames_read > 0 {
                        utils::array_fast_add_value_f32(
                            &self.buffer,
                            &mut self.resampler_buffer,
                            (frames_read as usize * channel_count) as usize,
                        );
                    }
                }
            }

            utils::array_fast_set_value_f32(&mut self.buffer, 0.0);
            utils::array_fast_set_value_f32(&mut self.temp_buffer, 0.0);

            for mixer in mixers.iter_mut() {
                if let Some(mut lock) = mixer.try_lock_poison() {
                    let frames_read = lock
                        .read_pcm_frames(
                            self.spatialization.as_mut(),
                            &mut self.buffer,
                            &mut self.temp_buffer,
                            required_frame_count,
                        )
                        .unwrap_or(0);

                    if frames_read > 0 {
                        utils::array_fast_add_value_f32(
                            &self.buffer,
                            &mut self.resampler_buffer,
                            (frames_read as usize * channel_count) as usize,
                        );
                    }
                }
            }
        }

        if !self.resampler.bypass_mode() {
            self.resampler.process(
                &self.resampler_buffer,
                required_frame_count,
                output,
                frame_count,
            )?;
        } else {
            utils::array_fast_copy_f32(
                &self.resampler_buffer,
                output,
                0,
                0,
                (required_frame_count as usize * channel_count) as usize,
            );
        }

        self.panner.process(output, &mut self.buffer, frame_count)?;
        self.volume.process(&self.buffer, output, frame_count)?;

        // Apply DSP callback if set
        if let Some(dsp_callback) = self.dsp_callback.as_ref() {
            dsp_callback(output, frame_count);
        }

        // divide by the number of channels and clip
        let num_of_sources = mixers.len() + channels.len();
        if num_of_sources > 1 {
            let output_sz = output.len();

            for i in 0..output_sz {
                output[i] /= num_of_sources as f32;
                output[i] = output[i].clamp(-1.0, 1.0);
            }
        }

        // Clean up stopped channels
        channels.retain(|channel| {
            if let Some(lock) = channel.try_lock_poison() {
                if lock.marked_as_deleted {
                    return false;
                }
            }

            true
        });

        mixers.retain(|mixer| {
            if let Some(lock) = mixer.try_lock_poison() {
                if lock.marked_as_deleted {
                    return false;
                }
            }

            true
        });

        Ok(())
    }

    pub fn add_channel(&mut self, channel: Arc<Mutex<AudioChannelInner>>) -> Result<(), String> {
        let mut channels = self.channels.lock_poison();
        channels.push(channel);
        Ok(())
    }

    pub fn remove_channel(&mut self, channel: usize) -> Result<(), String> {
        let mut channels = self.channels.lock_poison();
        if channel < channels.len() {
            channels.remove(channel);
            Ok(())
        } else {
            Err(format!("Channel with ID {} not found", channel))
        }
    }

    pub fn add_mixer(&mut self, mixer: Arc<Mutex<AudioMixerInner>>) -> Result<(), String> {
        let mut mixers = self.mixers.lock_poison();
        mixers.push(mixer);
        Ok(())
    }

    pub fn remove_mixer(&mut self, mixer: usize) -> Result<(), String> {
        let mut mixers = self.mixers.lock_poison();
        let mut index_to_remove = None;

        for (i, m) in mixers.iter().enumerate() {
            let locked = m.lock_poison();

            if locked.ref_id == mixer {
                index_to_remove = Some(i);
                break;
            }
        }

        if let Some(index) = index_to_remove {
            mixers.remove(index);
            Ok(())
        } else {
            Err(format!("Mixer with ID {} not found", mixer))
        }
    }
}

#[allow(non_snake_case)]
pub(crate) extern "C" fn audio_callback(
    _p: *mut ma_device,
    _pOutput: *mut std::ffi::c_void,
    _pInput: *const std::ffi::c_void,
    _frameCount: u32,
) {
    let result = std::panic::catch_unwind(|| {
        // SAFETY: All the pointers are valid and the function is called in a safe context.
        // The pointers were constructed by the miniaudio library and are valid for the duration of the callback
        // as long as the device is running and the array bounds within the frame count x channels are respected.
        unsafe {
            let device = &mut *_p;
            if device.pUserData.is_null() {
                return;
            }

            let inner = (device.pUserData as *mut AudioDeviceInner)
                .as_mut()
                .unwrap();

            let channel_count = device.playback.channels as usize;

            let output = std::slice::from_raw_parts_mut(
                _pOutput as *mut f32,
                _frameCount as usize * channel_count,
            );

            inner
                .process(output, _frameCount as u64)
                .unwrap_or_else(|err| {
                    eprintln!("Error processing audio: {}", err);
                });
        }
    });

    if let Err(err) = result {
        eprintln!("Rust panic! in audio callback: {:?}", err);
    }
}

impl Drop for AudioDeviceInner {
    fn drop(&mut self) {
        // SAFETY: This function is safe because it properly uninitializes the audio device and decoders.
        // The code ensures that all resources are released and cleaned up.
        unsafe {
            self.channels.lock_poison().clear();

            ma_device_uninit(self.device.as_mut());
        }
    }
}

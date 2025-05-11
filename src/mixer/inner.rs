use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use crate::{
    channel::inner::AudioChannelInner,
    effects::{AudioFX, AudioPanner, AudioResampler, AudioSpatializationListener, AudioVolume},
    utils::{self, MutexPoison},
};

use super::AudioMixerDSPCallback;

pub(crate) struct AudioChannelEntry {
    pub channel: Arc<Mutex<AudioChannelInner>>,
    pub delay: Option<u64>,
    pub duration: Option<u64>,
}

pub(crate) struct AudioMixerEntry {
    pub mixer: Arc<Mutex<AudioMixerInner>>,
    pub delay: Option<u64>,
    pub duration: Option<u64>,
}

pub(crate) struct AudioMixerInner {
    pub ref_id: usize,
    pub marked_as_deleted: bool,

    pub channels: Vec<AudioChannelEntry>,
    pub mixers: Vec<AudioMixerEntry>,
    pub is_playing: Arc<AtomicBool>,
    pub max_length: u64,
    pub mixer_position: u64,
    pub is_infinite: bool,
    pub dsp_callback: Option<AudioMixerDSPCallback>,

    pub channel_count: usize,
    pub sample_rate: u32,

    pub buffer: Vec<f32>,
    pub intermediate_buffer: Vec<f32>,

    pub resampler: AudioResampler,
    pub panner: AudioPanner,
    pub volume: AudioVolume,
    pub fx: Option<AudioFX>,
}

impl AudioMixerInner {
    pub fn new(channels: u32, sample_rate: u32, ref_id: usize) -> Result<Self, String> {
        let is_playing = Arc::new(AtomicBool::new(false));

        let resampler = AudioResampler::new(channels, sample_rate)?;
        let panner = AudioPanner::new(channels)?;
        let volume = AudioVolume::new(channels)?;

        let inner = AudioMixerInner {
            ref_id,
            marked_as_deleted: false,
            channels: Vec::new(),
            mixers: Vec::new(),
            is_playing: is_playing.clone(),
            max_length: 0,
            mixer_position: 0,
            is_infinite: false,
            dsp_callback: None,
            channel_count: channels as usize,
            sample_rate,
            buffer: vec![0.0; 4096 * channels as usize],
            intermediate_buffer: vec![0.0; 4096 * channels as usize],
            resampler,
            panner,
            volume,
            fx: None,
        };

        Ok(inner)
    }

    pub fn read_pcm_frames(
        &mut self,
        _spatialization: Option<&mut AudioSpatializationListener>,
        buffer: &mut [f32],
        temp_buffer: &mut [f32],
        frame_count: u64,
    ) -> Result<u64, String> {
        if !self.is_playing.load(Ordering::SeqCst) {
            return Ok(0);
        }

        let sample_count = frame_count as usize * self.channel_count;
        let required_frame_count = self
            .resampler
            .get_required_input(frame_count)
            .unwrap_or(0);

        let mut mixed_sources = 0;
        if self.fx.is_some() {

            let mut target_frame_count = required_frame_count;
            let mut readed_frame_count = required_frame_count;

            {
                let fx = self.fx.as_mut().unwrap();
                if !fx.tempo_bypass() {
                    target_frame_count = fx.get_required_input(target_frame_count).unwrap_or(0);
                }
            }

            let available_frames = self.max_length.saturating_sub(self.mixer_position);
            if available_frames > 0 {
                mixed_sources = self.mix_children_into_buffer(temp_buffer, target_frame_count)?;

                let fx = self.fx.as_mut().unwrap();

                if target_frame_count >= available_frames {
                    fx.frame_available += fx.get_output_latency() as i64;
                } else {
                    fx.frame_available += readed_frame_count as i64;
                }
            }

            let buffer = &mut self.buffer;
            let fx = self.fx.as_mut().unwrap();

            if fx.frame_available > 0 {
                fx.process(
                    buffer, 
                    target_frame_count, 
                    temp_buffer, 
                    readed_frame_count)?;

                fx.frame_available -= readed_frame_count as i64;

                if fx.frame_available < 0 {
                    readed_frame_count = (readed_frame_count as i64 + fx.frame_available) as u64;
                    fx.frame_available = 0;
                }
            } else {
                readed_frame_count = 0;
            }

            utils::array_fast_copy_f32(
                temp_buffer,
                buffer,
                0,
                0,
                (readed_frame_count * self.channel_count as u64) as usize,
            );
        } else {
            mixed_sources = self.mix_children_into_buffer(temp_buffer, required_frame_count)?;
        }

        if mixed_sources > 0 {
            if !self.resampler.bypass_mode() {
                self.resampler.process(
                    &self.buffer,
                    required_frame_count,
                    temp_buffer,
                    frame_count,
                )?;

                utils::array_fast_copy_f32(
                    temp_buffer,
                    &mut self.buffer,
                    0,
                    0,
                    (frame_count * self.channel_count as u64) as usize,
                );
            }

            self.panner.process(&self.buffer, temp_buffer, frame_count)?;
            self.volume.process(&temp_buffer, &mut self.buffer, frame_count)?;

            for i in 0..sample_count {
                buffer[i] /= mixed_sources as f32;
            }

            utils::array_fast_copy_f32(
                &self.buffer,
                buffer,
                0,
                0,
                sample_count,
            );
        }

        if self.dsp_callback.is_some() {
            let callback = self.dsp_callback.as_ref().unwrap();
            callback(buffer, frame_count);
        }

        if self.mixer_position >= self.max_length && !self.is_infinite {
            self.is_playing.store(false, Ordering::SeqCst);
        }

        Ok(frame_count)
    }

    fn mix_children_into_buffer(
        &mut self,
        temp_buffer: &mut [f32],
        frame_count: u64,
    ) -> Result<usize, String> {
        let mut mixed_sources = 0;
        let sample_count = frame_count as usize * self.channel_count;

        // Clear intermediate buffer
        for s in &mut self.buffer[..sample_count] {
            *s = 0.0;
        }

        for mx_channel in &mut self.channels {
            if let Some(mut channel) = mx_channel.channel.try_lock_poison() {
                let delay = mx_channel.delay.unwrap_or(0);
                let duration = mx_channel
                    .duration
                    .unwrap_or(channel.reader.pcm_length);

                if self.mixer_position < delay || self.mixer_position >= delay + duration {
                    continue;
                }

                let remaining_frames = (delay + duration).saturating_sub(self.mixer_position);
                let read_frames = frame_count.min(remaining_frames);

                let channel_frame_count = channel.read_pcm_frames(
                    None,
                    &mut self.intermediate_buffer,
                    temp_buffer,
                    read_frames,
                )?;

                if channel_frame_count > 0 {
                    mixed_sources += 1;

                    utils::array_fast_add_value_f32(
                        &self.intermediate_buffer,
                        &mut self.buffer,
                        (channel_frame_count * self.channel_count as u64) as usize,
                    );
                }
            }
        }

        for mx_mixer in &mut self.mixers {
            if let Some(mut mixer) = mx_mixer.mixer.try_lock_poison() {
                let delay = mx_mixer.delay.unwrap_or(0);
                let duration = mx_mixer.duration.unwrap_or(mixer.max_length);

                if self.mixer_position < delay || self.mixer_position >= delay + duration {
                    continue;
                }

                let remaining_frames = (delay + duration).saturating_sub(self.mixer_position);
                let read_frames = frame_count.min(remaining_frames);

                let mixer_frame_count = mixer.read_pcm_frames(
                    None,
                    &mut self.intermediate_buffer,
                    temp_buffer,
                    read_frames,
                )?;

                if mixer_frame_count > 0 {
                    mixed_sources += 1;

                    utils::array_fast_add_value_f32(
                        &self.intermediate_buffer,
                        &mut self.buffer,
                        (mixer_frame_count * self.channel_count as u64) as usize,
                    );
                }
            }
        }

        self.mixer_position += frame_count;

        Ok(mixed_sources)
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::SeqCst)
    }

    pub fn seek(&mut self, position: Option<u64>) -> Result<u64, String> {
        self.mixer_position = 0;
        let mut max_channel_seeked = 0;
        let position = position.unwrap_or(0);

        for mx_channel in &mut self.channels {
            if let Some(mut channel) = mx_channel.channel.try_lock_poison() {
                let delay = mx_channel.delay.unwrap_or(0);
                let duration = mx_channel.duration.unwrap_or(channel.reader.pcm_length);

                if position < delay {
                    continue;
                }

                let relative_position = position - delay;
                let limited_position = relative_position.min(duration);

                let channel_seeked = channel.seek(limited_position)?;
                if delay + channel_seeked > max_channel_seeked {
                    max_channel_seeked = delay + channel_seeked;
                }
            }
        }

        for mx_mixer in &mut self.mixers {
            if let Some(mut mixer) = mx_mixer.mixer.try_lock_poison() {
                let delay = mx_mixer.delay.unwrap_or(0);
                let duration = mx_mixer.duration.unwrap_or(mixer.max_length);

                if position < delay {
                    // Not yet time to play this mixer
                    continue;
                }

                let relative_position = position - delay;
                let limited_position = relative_position.min(duration);

                let mixer_seeked = mixer.seek(Some(limited_position))?;
                if delay + mixer_seeked > max_channel_seeked {
                    max_channel_seeked = delay + mixer_seeked;
                }
            }
        }

        if self.fx.is_some() {
            let input_latency = self.fx.as_ref().unwrap().get_input_latency();

            if input_latency > 0 {
                let mut temp_buffer = vec![0.0; (4096 * self.channel_count as u64) as usize];
                self.mix_children_into_buffer(&mut temp_buffer, input_latency as u64)?;

                let fx = self.fx.as_mut().unwrap();
                fx.pre_process(&self.buffer, input_latency as u64)?;

                fx.frame_available += input_latency as i64;
            }
        }

        Ok(max_channel_seeked)
    }

    fn compute_mixer_length(&mut self) -> Result<u64, String> {
        let mut max_length = 0;
        let mut has_infinite = false;

        // Handle channels
        for mx_channel in &self.channels {
            if let Some(channel) = mx_channel.channel.try_lock_poison() {
                let start_pcm = mx_channel.delay.unwrap_or(0);
                let actual_length = channel.reader.pcm_length;

                let duration = mx_channel.duration.unwrap_or(actual_length);
                let end_pcm = start_pcm + duration;

                has_infinite = has_infinite || channel.is_looping.load(Ordering::SeqCst);
                max_length = max_length.max(end_pcm);
            }
        }

        // Handle nested mixers
        for mx_mixer in self.mixers.iter_mut() {
            if let Some(mut mixer) = mx_mixer.mixer.try_lock_poison() {
                let start_pcm = mx_mixer.delay.unwrap_or(0);
                let nested_length = mixer.compute_mixer_length()?; // recursive

                let duration = mx_mixer.duration.unwrap_or(nested_length);
                let end_pcm = start_pcm + duration;

                has_infinite = has_infinite || mixer.is_infinite;
                max_length = max_length.max(end_pcm);
            }
        }

        self.max_length = max_length;
        self.is_infinite = has_infinite;
        Ok(max_length)
    }

    pub fn add_channel(
        &mut self,
        channel: Arc<Mutex<AudioChannelInner>>,
        delay: Option<u64>,
        duration: Option<u64>,
    ) -> Result<(), String> {
        let entry = AudioChannelEntry {
            channel,
            delay,
            duration,
        };

        self.channels.push(entry);
        self.compute_mixer_length()?;

        Ok(())
    }

    pub fn add_mixer(
        &mut self,
        mixer: Arc<Mutex<AudioMixerInner>>,
        delay: Option<u64>,
        duration: Option<u64>,
    ) -> Result<(), String> {
        let entry = AudioMixerEntry {
            mixer,
            delay,
            duration,
        };

        self.mixers.push(entry);
        self.compute_mixer_length()?;

        Ok(())
    }
}

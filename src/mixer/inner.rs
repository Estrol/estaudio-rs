use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, Ordering},
};

use crate::{
    effects::{
        AudioFX, AudioPanner, SpatializationListener, AudioVolume, ChannelConverter, Resampler,
    },
    math::{MathUtils, MathUtilsTrait},
    mixer::MixerError,
    sample::sampleinner::{SampleChannelHandle as SampleChannel, SampleChannelStatus},
    track::inner::TrackChannel,
};

#[derive(Debug)]
pub enum MixerEntry {
    TrackChannel {
        ref_id: usize,
        channel: Weak<Mutex<TrackChannel>>,
        delay: Option<usize>,
        duration: Option<usize>,
    },
    MixerChannel {
        ref_id: usize,
        mixer: Weak<Mutex<MixerChannel>>,
        delay: Option<usize>,
        duration: Option<usize>,
    },
    SampleChannel {
        ref_id: usize,
        channel: Weak<Mutex<SampleChannel>>,
        delay: Option<usize>,
        duration: Option<usize>,
    },
}

#[allow(dead_code)]
pub(crate) struct MixerChannel {
    pub ref_id: usize,
    pub marked_as_deleted: bool,
    pub normalize_output: bool,

    pub entries: Vec<MixerEntry>,
    pub is_playing: Arc<AtomicBool>,
    pub max_length: usize,
    pub mixer_position: usize,
    pub is_infinite: bool,
    pub dsp_callback: Option<Box<dyn FnMut(&[f32]) + Send + 'static>>,
    pub channel_converter: ChannelConverter,

    pub channel_count: usize,
    pub sample_rate: f32,

    pub buffer: Vec<f32>,
    pub intermediate_buffer: Vec<f32>,

    pub resampler: Resampler,
    pub panner: AudioPanner,
    pub volume: AudioVolume,
    pub fx: Option<AudioFX>,
}

impl std::fmt::Debug for MixerChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MixerChannel")
            .field("ref_id", &self.ref_id)
            .field("marked_as_deleted", &self.marked_as_deleted)
            .field("normalize_output", &self.normalize_output)
            .field("entries_count", &self.entries.len())
            .field("is_playing", &self.is_playing.load(Ordering::SeqCst))
            .field("max_length", &self.max_length)
            .field("mixer_position", &self.mixer_position)
            .field("is_infinite", &self.is_infinite)
            .field("channel_count", &self.channel_count)
            .field("sample_rate", &self.sample_rate)
            .finish()
    }
}

#[allow(dead_code)]
impl MixerChannel {
    pub fn new(channels: usize, sample_rate: f32, ref_id: usize) -> Result<Self, MixerError> {
        if channels < 1 || channels > 8 {
            return Err(MixerError::InvalidChannelCount(channels));
        }

        if sample_rate < 8000.0 || sample_rate > 192000.0 {
            return Err(MixerError::InvalidSampleRate(sample_rate as f32));
        }

        let is_playing = Arc::new(AtomicBool::new(false));

        let resampler = Resampler::new(channels, sample_rate).map_err(MixerError::from_other)?;

        let panner = AudioPanner::new(channels).map_err(MixerError::from_other)?;
        let volume = AudioVolume::new(channels).map_err(MixerError::from_other)?;
        let mut channel_converter = ChannelConverter::new();
        channel_converter.set_input_channels(channels as usize);

        let inner = MixerChannel {
            ref_id,
            marked_as_deleted: false,
            normalize_output: false,
            entries: Vec::new(),
            channel_converter,
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

    pub fn set_normalize_output(&mut self, value: bool) {
        self.normalize_output = value;
    }

    pub fn read(
        &mut self,
        _spatialization: Option<&mut SpatializationListener>,
        channel_converter: &mut ChannelConverter,
        buffer: &mut [f32],
        temp_buffer: &mut [f32],
        frame_count: usize,
    ) -> Result<usize, MixerError> {
        if !self.is_playing.load(Ordering::SeqCst) {
            return Ok(0);
        }

        let sample_count = frame_count as usize * self.channel_count;
        let required_frame_count = self.resampler.get_required_input(frame_count).unwrap_or(0);

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
                    fx.frame_available += fx.get_output_latency() as isize;
                } else {
                    fx.frame_available += readed_frame_count as isize;
                }
            }

            let buffer = &mut self.buffer;
            let fx = self.fx.as_mut().unwrap();

            if fx.frame_available > 0 {
                fx.process(
                    crate::macros::make_slice!(buffer, target_frame_count, self.channel_count),
                    crate::macros::make_slice_mut!(
                        temp_buffer,
                        target_frame_count,
                        self.channel_count
                    ),
                )
                .map_err(MixerError::from_other)?;

                fx.frame_available -= readed_frame_count as isize;

                if fx.frame_available < 0 {
                    readed_frame_count =
                        (readed_frame_count as isize + fx.frame_available) as usize;
                    fx.frame_available = 0;
                }
            } else {
                readed_frame_count = 0;
            }

            let sample_count =
                crate::macros::array_len_from!(readed_frame_count, self.channel_count);
            MathUtils::simd_copy(
                temp_buffer[..sample_count].as_ref(),
                buffer[..sample_count].as_mut(),
            );
        } else {
            mixed_sources = self.mix_children_into_buffer(temp_buffer, required_frame_count)?;
        }

        if mixed_sources > 0 {
            if !self.resampler.bypass_mode() {
                self.resampler
                    .process(
                        crate::macros::make_slice!(
                            self.buffer,
                            required_frame_count,
                            self.channel_count
                        ),
                        crate::macros::make_slice_mut!(
                            temp_buffer,
                            frame_count,
                            self.channel_count
                        ),
                    )
                    .map_err(MixerError::from_other)?;

                let sample_count = crate::macros::array_len_from!(frame_count, self.channel_count);
                MathUtils::simd_copy(
                    temp_buffer[..sample_count].as_ref(),
                    self.buffer[..sample_count].as_mut(),
                );
            }

            self.panner
                .process(&self.buffer, temp_buffer)
                .map_err(MixerError::from_other)?;

            self.volume
                .process(&temp_buffer, &mut self.buffer)
                .map_err(MixerError::from_other)?;

            if self.normalize_output {
                for i in 0..sample_count {
                    buffer[i] /= mixed_sources as f32;
                }
            }

            let size = crate::macros::array_len_from!(frame_count, self.channel_count);
            MathUtils::simd_copy(self.buffer[..size].as_ref(), buffer[..size].as_mut());
        }

        if let Some(callback) = self.dsp_callback.as_mut() {
            callback(&buffer[..sample_count]);
        }

        if self.mixer_position >= self.max_length && !self.is_infinite {
            self.is_playing.store(false, Ordering::SeqCst);
        }

        self.channel_converter
            .set_input_channels(self.channel_count);
        self.channel_converter.process(buffer, temp_buffer);

        channel_converter.set_input_channels(self.channel_count);
        channel_converter.process(temp_buffer, buffer);

        Ok(frame_count)
    }

    fn mix_children_into_buffer(
        &mut self,
        temp_buffer: &mut [f32],
        frame_count: usize,
    ) -> Result<usize, MixerError> {
        let mut mixed_sources = 0;
        let sample_count = frame_count as usize * self.channel_count;

        // Clear intermediate buffer
        MathUtils::simd_set(self.buffer[..sample_count].as_mut(), 0.0);

        for entry in self.entries.iter_mut() {
            match entry {
                MixerEntry::TrackChannel {
                    channel,
                    delay,
                    duration,
                    ..
                } => {
                    let Some(channel) = channel.upgrade() else {
                        continue;
                    };

                    let Ok(mut channel) = channel.try_lock() else {
                        continue;
                    };

                    if self.mixer_position < delay.unwrap_or(0)
                        || self.mixer_position
                            >= delay.unwrap_or(0) + duration.unwrap_or(channel.reader.pcm_length)
                    {
                        continue;
                    }

                    let remaining_frames = (delay.unwrap_or(0)
                        + duration.unwrap_or(channel.reader.pcm_length))
                    .saturating_sub(self.mixer_position);

                    let read_frames = frame_count.min(remaining_frames);

                    let channel_frame_count = channel
                        .read(
                            None,
                            &mut self.channel_converter,
                            &mut self.intermediate_buffer,
                            temp_buffer,
                            read_frames,
                        )
                        .map_err(MixerError::from_other)?;

                    if channel_frame_count > 0 {
                        let size =
                            crate::macros::array_len_from!(channel_frame_count, self.channel_count);

                        mixed_sources +=
                            MathUtils::simd_not_any(&self.intermediate_buffer[..size], 0.0)
                                as usize;

                        MathUtils::simd_add(
                            self.intermediate_buffer[..size].as_mut(),
                            self.buffer[..size].as_ref(),
                        );
                    }
                }
                MixerEntry::MixerChannel {
                    mixer,
                    delay,
                    duration,
                    ..
                } => {
                    let Some(mixer) = mixer.upgrade() else {
                        continue;
                    };

                    let Ok(mut mixer) = mixer.try_lock() else {
                        continue;
                    };

                    if self.mixer_position < delay.unwrap_or(0)
                        || self.mixer_position
                            >= delay.unwrap_or(0) + duration.unwrap_or(mixer.max_length)
                    {
                        continue;
                    }

                    let remaining_frames = (delay.unwrap_or(0)
                        + duration.unwrap_or(mixer.max_length))
                    .saturating_sub(self.mixer_position);

                    let read_frames = frame_count.min(remaining_frames);

                    let mixer_frame_count = mixer.read(
                        None,
                        &mut self.channel_converter,
                        &mut self.intermediate_buffer,
                        temp_buffer,
                        read_frames,
                    )?;

                    if mixer_frame_count > 0 {
                        let size =
                            crate::macros::array_len_from!(mixer_frame_count, self.channel_count);

                        mixed_sources +=
                            MathUtils::simd_not_any(&self.intermediate_buffer[..size], 0.0)
                                as usize;

                        MathUtils::simd_add(
                            self.intermediate_buffer[..size].as_mut(),
                            self.buffer[..size].as_ref(),
                        );
                    }
                }
                MixerEntry::SampleChannel {
                    channel,
                    delay,
                    duration,
                    ..
                } => {
                    let Some(channel) = channel.upgrade() else {
                        continue;
                    };

                    let Ok(mut channel) = channel.try_lock() else {
                        continue;
                    };

                    if self.mixer_position < delay.unwrap_or(0)
                        || self.mixer_position
                            >= delay.unwrap_or(0) + duration.unwrap_or(channel.reader.pcm_length)
                    {
                        continue;
                    }

                    let remaining_frames = (delay.unwrap_or(0)
                        + duration.unwrap_or(channel.reader.pcm_length))
                    .saturating_sub(self.mixer_position);

                    let read_frames = frame_count.min(remaining_frames);
                    let channel_frame_count = channel
                        .read(
                            None,
                            &mut self.channel_converter,
                            &mut self.intermediate_buffer,
                            temp_buffer,
                            read_frames,
                        )
                        .map_err(MixerError::from_other)?;

                    if channel_frame_count > 0 {
                        let size =
                            crate::macros::array_len_from!(channel_frame_count, self.channel_count);

                        mixed_sources +=
                            MathUtils::simd_not_any(&self.intermediate_buffer[..size], 0.0)
                                as usize;

                        MathUtils::simd_add(
                            self.intermediate_buffer[..size].as_mut(),
                            self.buffer[..size].as_ref(),
                        );
                    }
                }
            }
        }

        self.mixer_position += frame_count;

        Ok(mixed_sources)
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::SeqCst)
    }

    pub fn seek(&mut self, position: Option<usize>) -> Result<usize, MixerError> {
        self.mixer_position = 0;
        let mut max_channel_seeked = 0;
        let position = position.unwrap_or(0);

        for entry in self.entries.iter_mut() {
            match entry {
                MixerEntry::TrackChannel {
                    channel,
                    delay,
                    duration,
                    ..
                } => {
                    let Some(channel) = channel.upgrade() else {
                        continue;
                    };

                    let Ok(mut channel) = channel.lock() else {
                        continue;
                    };

                    let delay = delay.unwrap_or(0);
                    let duration = duration.unwrap_or(channel.reader.pcm_length);

                    if position < delay {
                        continue;
                    }

                    let relative_position = position - delay;
                    let limited_position = relative_position.min(duration);

                    let channel_seeked = channel
                        .seek(limited_position)
                        .map_err(MixerError::from_other)?;

                    if delay + channel_seeked > max_channel_seeked {
                        max_channel_seeked = delay + channel_seeked;
                    }
                }
                MixerEntry::SampleChannel {
                    channel,
                    delay,
                    duration,
                    ..
                } => {
                    let Some(channel) = channel.upgrade() else {
                        continue;
                    };

                    let Ok(mut channel) = channel.lock() else {
                        continue;
                    };

                    let delay = delay.unwrap_or(0);
                    let duration = duration.unwrap_or(channel.reader.pcm_length);

                    if position < delay {
                        continue;
                    }

                    let relative_position = position - delay;
                    let limited_position = relative_position.min(duration);

                    let channel_seeked = channel
                        .seek(limited_position)
                        .map_err(MixerError::from_other)?;

                    if delay + channel_seeked > max_channel_seeked {
                        max_channel_seeked = delay + channel_seeked;
                    }
                }
                MixerEntry::MixerChannel {
                    mixer,
                    delay,
                    duration,
                    ..
                } => {
                    let Some(mixer) = mixer.upgrade() else {
                        continue;
                    };

                    let Ok(mut mixer) = mixer.lock() else {
                        continue;
                    };

                    let delay = delay.unwrap_or(0);
                    let duration = duration.unwrap_or(mixer.max_length);

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
        }

        // Well, if infinite we have to eat the latency
        if self.fx.is_some() && !self.is_infinite {
            let input_latency = {
                let fx = self.fx.as_mut().unwrap();
                fx.configure(self.max_length)
                    .map_err(MixerError::from_other)?
            };

            if input_latency > 0 {
                let mut temp_buffer = vec![0.0; (input_latency as usize) * self.channel_count];
                self.mix_children_into_buffer(&mut temp_buffer, input_latency)?;

                let fx = self.fx.as_mut().unwrap();
                fx.seek(&temp_buffer).map_err(MixerError::from_other)?;

                fx.frame_available += input_latency as isize;
            }
        }

        Ok(max_channel_seeked)
    }

    fn compute_mixer_length(&mut self) -> Result<usize, MixerError> {
        let mut max_length = 0;
        let mut has_infinite = false;

        for entry in self.entries.iter() {
            match entry {
                MixerEntry::TrackChannel {
                    channel,
                    delay,
                    duration,
                    ..
                } => {
                    let Some(channel) = channel.upgrade() else {
                        continue;
                    };

                    let Ok(channel) = channel.try_lock() else {
                        continue;
                    };

                    let start_pcm = delay.unwrap_or(0);
                    let actual_length = channel.reader.pcm_length;

                    let duration = duration.unwrap_or(actual_length);
                    let end_pcm = start_pcm + duration;

                    has_infinite = has_infinite || channel.is_looping.load(Ordering::SeqCst);
                    max_length = max_length.max(end_pcm);
                }
                MixerEntry::SampleChannel {
                    channel,
                    delay,
                    duration,
                    ..
                } => {
                    let Some(channel) = channel.upgrade() else {
                        continue;
                    };

                    let Ok(channel) = channel.try_lock() else {
                        continue;
                    };

                    let start_pcm = delay.unwrap_or(0);
                    let actual_length = channel.reader.pcm_length;

                    let duration = duration.unwrap_or(actual_length);
                    let end_pcm = start_pcm + duration;

                    max_length = max_length.max(end_pcm);
                }
                MixerEntry::MixerChannel {
                    mixer,
                    delay,
                    duration,
                    ..
                } => {
                    let Some(mixer) = mixer.upgrade() else {
                        continue;
                    };

                    let Ok(mixer) = mixer.try_lock() else {
                        continue;
                    };

                    let start_pcm = delay.unwrap_or(0);
                    let actual_length = mixer.max_length;

                    let duration = duration.unwrap_or(actual_length);
                    let end_pcm = start_pcm + duration;

                    has_infinite = has_infinite || mixer.is_infinite;
                    max_length = max_length.max(end_pcm);
                }
            }
        }

        self.max_length = max_length;
        self.is_infinite = has_infinite;
        Ok(max_length)
    }

    pub fn add_track(
        &mut self,
        channel: Weak<Mutex<TrackChannel>>,
        delay: Option<usize>,
        duration: Option<usize>,
    ) -> Result<(), MixerError> {
        let Some(channel_up) = channel.upgrade() else {
            return Err(MixerError::InvalidOperation(
                "Failed to upgrade TrackChannel",
            ));
        };

        let ref_id = match channel_up.lock() {
            Ok(channel) => channel.ref_id,
            Err(_) => return Err(MixerError::InvalidOperation("Failed to lock TrackChannel")),
        };

        let entry = MixerEntry::TrackChannel {
            ref_id: ref_id,
            channel: channel,
            delay,
            duration,
        };

        self.entries.push(entry);
        self.compute_mixer_length()?;

        Ok(())
    }

    pub fn add_mixer(
        &mut self,
        mixer: Weak<Mutex<MixerChannel>>,
        delay: Option<usize>,
        duration: Option<usize>,
    ) -> Result<(), MixerError> {
        let Some(mixer_up) = mixer.upgrade() else {
            return Err(MixerError::InvalidOperation(
                "Failed to upgrade MixerChannel",
            ));
        };

        let ref_id = match mixer_up.lock() {
            Ok(mixer) => mixer.ref_id,
            Err(_) => return Err(MixerError::InvalidOperation("Failed to lock MixerChannel")),
        };

        let entry = MixerEntry::MixerChannel {
            ref_id,
            mixer,
            delay,
            duration,
        };

        self.entries.push(entry);
        self.compute_mixer_length()?;

        Ok(())
    }

    pub fn remove_track(&mut self, channel: &Weak<Mutex<TrackChannel>>) -> Result<(), MixerError> {
        let Some(channel_up) = channel.upgrade() else {
            return Err(MixerError::InvalidOperation(
                "Failed to upgrade TrackChannel",
            ));
        };

        let ref_id = match channel_up.lock() {
            Ok(channel) => channel.ref_id,
            Err(_) => return Err(MixerError::InvalidOperation("Failed to lock TrackChannel")),
        };

        if let Some(index) = self.entries.iter().position(|entry| {
            matches!(entry, MixerEntry::TrackChannel { ref_id: entry_ref_id, .. } if *entry_ref_id == ref_id)
        }) {
            self.entries.remove(index);
            self.compute_mixer_length()?;
            Ok(())
        } else {
            Err(MixerError::InvalidOperation("Track not found in mixer"))
        }
    }

    pub fn remove_sample(&mut self, channel: &Weak<Mutex<SampleChannel>>) -> Result<(), MixerError> {
        let Some(channel_up) = channel.upgrade() else {
            return Err(MixerError::InvalidOperation(
                "Failed to upgrade SampleChannel",
            ));
        };

        let ref_id = match channel_up.lock() {
            Ok(channel) => channel.ref_id,
            Err(_) => return Err(MixerError::InvalidOperation("Failed to lock SampleChannel")),
        };

        if let Some(index) = self.entries.iter().position(|entry| {
            matches!(entry, MixerEntry::SampleChannel { ref_id: entry_ref_id, .. } if *entry_ref_id == ref_id)
        }) {
            self.entries.remove(index);
            self.compute_mixer_length()?;
            Ok(())
        } else {
            Err(MixerError::InvalidOperation("Sample not found in mixer"))
        }
    }

    pub fn remove_mixer(&mut self, mixer: &Weak<Mutex<MixerChannel>>) -> Result<(), MixerError> {
        let Some(mixer_up) = mixer.upgrade() else {
            return Err(MixerError::InvalidOperation(
                "Failed to upgrade MixerChannel",
            ));
        };

        let ref_id = match mixer_up.lock() {
            Ok(mixer) => mixer.ref_id,
            Err(_) => return Err(MixerError::InvalidOperation("Failed to lock MixerChannel")),
        };

        if let Some(index) = self.entries.iter().position(|entry| {
            matches!(entry, MixerEntry::MixerChannel { ref_id: entry_ref_id, .. } if *entry_ref_id == ref_id)
        }) {
            self.entries.remove(index);
            self.compute_mixer_length()?;
            Ok(())
        } else {
            Err(MixerError::InvalidOperation("Mixer not found in mixer"))
        }
    }

    pub fn add_sample(
        &mut self,
        channel: Weak<Mutex<SampleChannel>>,
        delay: Option<usize>,
        duration: Option<usize>,
    ) -> Result<(), MixerError> {
        let Some(channel_up) = channel.upgrade() else {
            return Err(MixerError::InvalidOperation(
                "Failed to upgrade SampleChannel",
            ));
        };

        let ref_id = match channel_up.lock() {
            Ok(channel) => channel.ref_id,
            Err(_) => return Err(MixerError::InvalidOperation("Failed to lock SampleChannel")),
        };

        let entry = MixerEntry::SampleChannel {
            ref_id,
            channel,
            delay,
            duration,
        };

        self.entries.push(entry);
        self.compute_mixer_length()?;

        Ok(())
    }

    pub fn set_callback<F>(&mut self, callback: F) -> Result<(), MixerError>
    where
        F: FnMut(&[f32]) + Send + 'static,
    {
        self.dsp_callback = Some(Box::new(callback));
        Ok(())
    }

    pub fn start(&mut self) {
        Self::recursive_play(self, true, 0);
    }

    pub fn stop(&mut self) {
        Self::recursive_play(self, false, 0);
    }

    pub fn recursive_play(mixer: &mut MixerChannel, playing: bool, depth: usize) {
        const MAX_DEPTH: usize = 16;

        if depth > MAX_DEPTH {
            eprintln!("Maximum mixer recursion depth exceeded");
            return;
        }

        mixer.is_playing.store(playing, Ordering::SeqCst);

        for entry in mixer.entries.iter() {
            match entry {
                MixerEntry::TrackChannel { channel, .. } => {
                    let Some(channel) = channel.upgrade() else {
                        continue;
                    };

                    let Ok(channel) = channel.lock() else {
                        continue;
                    };

                    channel.playing.store(playing, Ordering::SeqCst);
                }
                MixerEntry::SampleChannel { channel, .. } => {
                    let Some(channel) = channel.upgrade() else {
                        continue;
                    };

                    let Ok(channel) = channel.lock() else {
                        continue;
                    };

                    channel.status.store(
                        match playing {
                            true => SampleChannelStatus::Playing,
                            false => SampleChannelStatus::NotStarted,
                        },
                        Ordering::SeqCst,
                    );
                }
                MixerEntry::MixerChannel { mixer, .. } => {
                    let Some(mixer) = mixer.upgrade() else {
                        continue;
                    };

                    let Ok(mut mixer) = mixer.lock() else {
                        continue;
                    };

                    Self::recursive_play(&mut mixer, playing, depth + 1);
                }
            }
        }
    }
}

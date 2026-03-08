use crate::{
    BufferInfo,
    audioreader::{AudioReader, cache::AudioCache},
    effects::{
        AudioFX, AudioPanner, Spatialization, SpatializationListener, AudioVolume,
        ChannelConverter, Resampler,
    },
    math::{MathUtils, MathUtilsTrait},
    track::TrackError,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Instant,
};

#[allow(dead_code)]
pub(crate) struct TrackChannel {
    pub ref_id: usize,
    pub marked_as_deleted: bool,

    pub reader: AudioReader,
    pub last_time: Instant,

    pub gainer: AudioVolume,
    pub panner: AudioPanner,
    pub resampler: Resampler,
    pub channel_converter: ChannelConverter,
    pub fx: Option<AudioFX>,

    pub playing: Arc<AtomicBool>,
    pub is_looping: Arc<AtomicBool>,
    pub position: Arc<AtomicUsize>,

    pub spatializer: Option<Spatialization>,
    pub callback: Option<Box<dyn FnMut(&mut [f32]) + Send + 'static>>,

    pub start: Option<usize>,
    pub end: Option<usize>,
}

impl std::fmt::Debug for TrackChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrackChannel")
            .field("ref_id", &self.ref_id)
            .field("marked_as_deleted", &self.marked_as_deleted)
            .field("reader", &"AudioReader { ... }")
            .field("last_time", &self.last_time)
            .field("gainer", &"AudioVolume { ... }")
            .field("panner", &"AudioPanner { ... }")
            .field("resampler", &"Resampler { ... }")
            .field("channel_converter", &"ChannelConverter { ... }")
            .field("fx", &self.fx.as_ref().map(|_| "AudioFX { ... }"))
            .field("playing", &self.playing.load(Ordering::SeqCst))
            .field("is_looping", &self.is_looping.load(Ordering::SeqCst))
            .field("position", &self.position.load(Ordering::SeqCst))
            .field(
                "spatializer",
                &self
                    .spatializer
                    .as_ref()
                    .map(|_| "AudioSpatialization { ... }"),
            )
            .finish()
    }
}

#[allow(dead_code)]
impl TrackChannel {
    pub fn new(
        ref_id: usize,
        cache: Option<Arc<AudioCache>>,
        buffer: Option<BufferInfo>,
        sample_rate: Option<f32>,
        channels: Option<usize>,
        owned: bool,
    ) -> Result<Self, TrackError> {
        let reader = if let Some(cache_key) = cache {
            crate::macros::check!(AudioReader::load_cache(cache_key), TrackError::CreateFailed)
        } else if let Some(buffer_info) = buffer {
            crate::macros::check!(
                AudioReader::load_audio_buffer(
                    buffer_info.data,
                    buffer_info.sample_rate,
                    buffer_info.channels,
                    buffer_info.data.len() / buffer_info.channels,
                    owned
                ),
                TrackError::CreateFailed
            )
        } else {
            return Err(TrackError::CreateFailed);
        };

        let panner = crate::macros::check!(AudioPanner::new(reader.channels), TrackError::CreateFailed);
        let gainer = crate::macros::check!(AudioVolume::new(reader.channels), TrackError::CreateFailed);
        let mut resampler = crate::macros::check!(
            Resampler::new(reader.channels, reader.sample_rate),
            TrackError::CreateFailed
        );
        let mut channel_converter = ChannelConverter::new();

        let channels = channels.unwrap_or(reader.channels);
        let sample_rate = sample_rate.unwrap_or(reader.sample_rate);

        channel_converter.set_output_channels(channels as usize);
        channel_converter.set_input_channels(reader.channels as usize);
        resampler.set_target_sample_rate(sample_rate);

        let atomic_playing = Arc::new(AtomicBool::new(false));
        let atomic_position = Arc::new(AtomicUsize::new(0));
        let atomic_is_looping = Arc::new(AtomicBool::new(false));

        Ok(Self {
            ref_id,
            marked_as_deleted: false,
            reader,
            last_time: Instant::now(),
            gainer,
            panner,
            resampler,
            channel_converter,
            fx: None,
            playing: atomic_playing,
            is_looping: atomic_is_looping,
            position: atomic_position,
            spatializer: None,
            callback: None,
            start: None,
            end: None,
        })
    }

    pub fn read(
        &mut self,
        spatializer_listener: Option<&mut SpatializationListener>,
        channel_converter: &mut ChannelConverter,
        output: &mut [f32],
        buffer1: &mut [f32],
        frame_count: usize,
    ) -> Result<usize, TrackError> {
        if !self.playing.load(Ordering::SeqCst) {
            return Ok(0);
        }

        let required_frame_count = self.resampler.get_required_input(frame_count).unwrap_or(0);
        if required_frame_count == 0 {
            return Ok(0);
        }

        let mut frames_readed;

        if self.fx.is_some() {
            let fx = self.fx.as_mut().unwrap();

            let mut target_frame_count = required_frame_count;
            let mut readed_frame_count = required_frame_count;

            if !fx.tempo_bypass() {
                target_frame_count = fx.get_required_input(target_frame_count).unwrap_or(0);
            }

            let available_frames = self.reader.available_frames();
            if available_frames > 0 {
                target_frame_count = crate::macros::check!(
                    self.reader.read(crate::macros::make_slice_mut!(
                        buffer1,
                        target_frame_count,
                        self.reader.channels
                    )),
                    TrackError::ReadError
                );

                if target_frame_count >= available_frames {
                    fx.frame_available += fx.get_output_latency() as isize;
                } else {
                    fx.frame_available += readed_frame_count as isize;
                }
            }

            if fx.frame_available > 0 {
                crate::macros::check!(
                    fx.process(
                        crate::macros::make_slice!(
                            buffer1,
                            target_frame_count,
                            self.reader.channels
                        ),
                        crate::macros::make_slice_mut!(
                            output,
                            readed_frame_count,
                            self.reader.channels
                        ),
                    ),
                    TrackError::ProcessingFailed
                );

                fx.frame_available -= readed_frame_count as isize;

                if fx.frame_available < 0 {
                    readed_frame_count =
                        (readed_frame_count as isize + fx.frame_available) as usize;
                    fx.frame_available = 0;
                }
            } else {
                readed_frame_count = 0;
            }

            frames_readed = readed_frame_count;
        } else {
            frames_readed = crate::macros::check!(
                self.reader.read(crate::macros::make_slice_mut!(
                    output[..crate::macros::array_len_from!(
                        required_frame_count,
                        self.reader.channels
                    )],
                    required_frame_count,
                    self.reader.channels
                ),),
                TrackError::ReadError
            );
        }

        if frames_readed > 0 {
            if !self.resampler.bypass_mode() {
                let resampler_frame_count = crate::macros::check!(
                    self.resampler.process(
                        crate::macros::make_slice!(output, frames_readed, self.reader.channels),
                        crate::macros::make_slice_mut!(buffer1, frame_count, self.reader.channels),
                    ),
                    TrackError::ProcessingFailed
                );

                let size = (resampler_frame_count * self.reader.channels) as usize;
                MathUtils::simd_copy(buffer1[..size].as_ref(), output[..size].as_mut());

                frames_readed = frame_count;
            }

            let buffer1 =
                crate::macros::make_slice_mut!(buffer1, frames_readed, self.reader.channels);
            let output =
                crate::macros::make_slice_mut!(output, frames_readed, self.reader.channels);

            crate::macros::check!(
                self.gainer.process(output, buffer1),
                TrackError::ProcessingFailed
            );
            crate::macros::check!(
                self.panner.process(buffer1, output),
                TrackError::ProcessingFailed
            );

            // User desired channels conversion
            self.channel_converter
                .set_input_channels(self.reader.channels as usize);
            self.channel_converter.process(output, buffer1);

            // Caller desired channels conversion
            channel_converter
                .set_input_channels(self.channel_converter.get_output_channels() as usize);
            channel_converter.process(buffer1, output);

            self.position.fetch_add(frames_readed, Ordering::SeqCst);

            if let Some(callback) = &mut self.callback {
                callback(output);
            }

            if let Some(spatializer) = &mut self.spatializer {
                if let Some(listener) = spatializer_listener {
                    crate::macros::check!(
                        spatializer.process(listener, output, buffer1),
                        TrackError::ProcessingFailed
                    );

                    MathUtils::simd_copy(buffer1.as_ref(), output.as_mut());
                }
            }
        }

        if frames_readed < frame_count {
            if self.is_looping.load(Ordering::SeqCst) {
                crate::macros::check!(self.reader.seek(0), TrackError::SeekFailed);
            } else {
                self.playing.store(false, Ordering::SeqCst);
            }
        }

        return Ok(frames_readed);
    }

    pub fn seek(&mut self, position: usize) -> Result<usize, TrackError> {
        if position >= self.reader.pcm_length {
            return Err(TrackError::SeekOutOfBounds);
        }

        self.position.store(position, Ordering::SeqCst);

        crate::macros::check!(self.reader.seek(position), TrackError::SeekFailed);

        if self.fx.is_some() {
            let fx = self.fx.as_mut().unwrap();
            let latency = crate::macros::check_ret!(
                fx.configure(self.reader.pcm_length),
                TrackError::from_other
            );

            // Only seek when need to feed the fx
            if latency > 0 {
                let mut input_buffer =
                    vec![0.0f32; latency as usize * self.reader.channels as usize];

                crate::macros::check!(self.reader.read(&mut input_buffer), TrackError::ReadError);
                crate::macros::check!(fx.seek(&input_buffer), TrackError::ProcessingFailed);

                fx.frame_available += latency as isize;
            }
        }

        Ok(position)
    }

    pub fn is_playing(&self) -> bool {
        self.playing.load(Ordering::SeqCst)
    }

    pub fn set_callback<F>(&mut self, callback: F)
    where
        F: FnMut(&mut [f32]) + Send + 'static,
    {
        self.callback = Some(Box::new(callback));
    }
}

impl Drop for TrackChannel {
    fn drop(&mut self) {
        self.playing.store(false, Ordering::SeqCst);
    }
}

use std::sync::{Arc, atomic::Ordering};

use thiserror::Error;

use crate::{
    audioreader::{AudioReader, cache::AudioCache},
    effects::{
        AudioFX, AudioPanner, SpatializationListener, AudioVolume, ChannelConverter, Resampler,
    },
    math::{MathUtils, MathUtilsTrait as _}, utils,
};

#[derive(Debug)]
pub struct SampleChannelHandle {
    pub(crate) ref_id: usize,
    pub(crate) reader: AudioReader,

    pub(crate) volume: AudioVolume,
    pub(crate) panner: AudioPanner,
    pub(crate) resampler: Resampler,
    pub(crate) channel_converter: ChannelConverter,
    pub(crate) fx: Option<AudioFX>,

    pub(crate) status: Arc<AtomicSampleChannelStatus>,
}

impl SampleChannelHandle {
    pub(crate) fn new(
        cache: &Option<Arc<AudioCache>>,
        buffer: &Option<crate::BufferInfo>,
        channel: usize,
        sample_rate: f32,
    ) -> Result<Self, SampleChannelError> {
        let reader = if let Some(cache_key) = cache {
            AudioReader::load_cache(Arc::clone(cache_key))
        } else if let Some(buffer_info) = buffer {
            let pcm_length = buffer_info.data.len() / buffer_info.channels;

            AudioReader::load_audio_buffer(
                buffer_info.data,
                buffer_info.sample_rate,
                buffer_info.channels,
                pcm_length,
                false,
            )
        } else {
            return Err(SampleChannelError::MissingAudioSource);
        };

        if let Err(e) = reader {
            return Err(SampleChannelError::from_other(e));
        }

        let reader = reader.unwrap();

        let volume = crate::macros::check_ret!(
            AudioVolume::new(reader.channels),
            SampleChannelError::from_other
        );
        
        let panner = crate::macros::check_ret!(
            AudioPanner::new(reader.channels),
            SampleChannelError::from_other
        );

        let mut resampler = crate::macros::check_ret!(
            Resampler::new(reader.channels, reader.sample_rate),
            SampleChannelError::from_other
        );

        resampler.set_target_sample_rate(sample_rate);

        let mut channel_converter = ChannelConverter::new();
        channel_converter.set_output_channels(channel as usize);

        let status = Arc::new(AtomicSampleChannelStatus::new(SampleChannelStatus::NotStarted));
        
        Ok(Self {
            ref_id: 0,
            reader,
            volume,
            panner,
            resampler,
            channel_converter,
            fx: None,
            status,
        })
    }

    pub fn read(
        &mut self,
        spatializer_listener: Option<&mut SpatializationListener>,
        channel_converter: &mut ChannelConverter,
        output: &mut [f32],
        buffer1: &mut [f32],
        frame_count: usize,
    ) -> Result<usize, SampleChannelError> {
        if self.status.load(Ordering::Relaxed) != SampleChannelStatus::Playing {
            return Ok(0);
        }

        if frame_count == 0 {
            return Ok(0);
        }

        let required_frame_count = self.resampler.get_required_input(frame_count).unwrap_or(0);

        if required_frame_count == 0 {
            return Ok(0);
        }

        let readed_frames = crate::macros::check_ret!(
            self.reader.read(crate::macros::make_slice_mut!(
                buffer1,
                required_frame_count,
                self.reader.channels
            )),
            SampleChannelError::from_other
        );

        if readed_frames > 0 {
            // resampler pass
            if !self.resampler.bypass_mode() {
                crate::macros::check_ret!(
                    self.resampler.process(
                        crate::macros::make_slice!(
                            buffer1,
                            readed_frames,
                            self.reader.channels
                        ),
                        crate::macros::make_slice_mut!(
                            output,
                            frame_count,
                            self.reader.channels
                        ),
                    ),
                    SampleChannelError::from_other
                );

                let size = frame_count as usize * self.reader.channels as usize;
                MathUtils::simd_copy(buffer1[..size].as_ref(), output[..size].as_mut());
            }

            // volume and panner pass
            {
                let buffer1 =
                    crate::macros::make_slice_mut!(buffer1, readed_frames, self.reader.channels);
                let output =
                    crate::macros::make_slice_mut!(output, frame_count, self.reader.channels);

                crate::macros::check_ret!(
                    self.volume.process(output, buffer1),
                    SampleChannelError::from_other
                );

                crate::macros::check_ret!(
                    self.panner.process(buffer1, output),
                    SampleChannelError::from_other
                );
            }

            // spatialization pass
            if let Some(listener) = spatializer_listener {
                _ = listener; // TODO:
            }

            // channel conversion pass
            {
                self.channel_converter
                    .set_input_channels(self.reader.channels as usize);

                // Self conversion
                {
                    let src = crate::macros::make_slice!(
                        output,
                        readed_frames,
                        self.channel_converter.get_input_channels()
                    );
                    let dst = crate::macros::make_slice_mut!(
                        buffer1,
                        frame_count,
                        self.channel_converter.get_output_channels()
                    );

                    self.channel_converter.process(src, dst);
                }

                channel_converter.set_input_channels(self.channel_converter.get_output_channels());

                // Caller conversion
                {
                    let src = crate::macros::make_slice!(
                        buffer1,
                        frame_count,
                        channel_converter.get_input_channels()
                    );
                    let dst = crate::macros::make_slice_mut!(
                        output,
                        frame_count,
                        channel_converter.get_output_channels()
                    );

                    channel_converter.process(src, dst);
                }
            }
        } else {
            self.status
                .store(SampleChannelStatus::Finished, Ordering::Relaxed);
        }

        Ok(readed_frames)
    }

    pub fn seek(&mut self, position: usize) -> Result<usize, SampleChannelError> {
        if position >= self.reader.pcm_length {
            return Err(SampleChannelError::SeekOutOfBounds(position));
        }

        crate::macros::check_ret!(
            self.reader.seek(position),
            SampleChannelError::from_other
        );

        self.status
            .store(SampleChannelStatus::Playing, Ordering::Relaxed);

        if let Some(fx) = &mut self.fx {
            let latency = crate::macros::check_ret!(
                fx.configure(self.reader.pcm_length),
                SampleChannelError::from_other
            );
            
            let mut data = vec![0.0; latency * self.reader.channels];

            crate::macros::check_ret!(
                self.reader.read(crate::macros::make_slice_mut!(
                    data,
                    latency as u64,
                    self.reader.channels
                )),
                SampleChannelError::from_other
            );

            crate::macros::check_ret!(
                fx.seek(&data),
                SampleChannelError::from_other
            );
        }

        Ok(position)
    }
}

#[derive(Debug, Error)]
pub enum SampleChannelError {
    #[error("Invalid device reference ID: {0}")]
    InvalidDeviceRefId(u32),
    #[error("Missing audio data source (buffer or cache)")]
    MissingAudioSource,
    #[error("Sample channel init failed: {} {}", .0, self.ma_error_to_str())]
    InitFailed(i32),
    #[error("Seek position {0} is out of bounds for the audio data")]
    SeekOutOfBounds(usize),
    #[error("Failed to lock Sample")]
    LockFailed,
    #[error("{0}")]
    Other(Box<dyn std::error::Error + Send + 'static>),
}

impl SampleChannelError {
    fn ma_error_to_str(&self) -> &str {
        match self {
            SampleChannelError::InitFailed(code) => utils::ma_to_string_result(*code),
            _ => "",
        }
    }

    pub fn from_other<E: std::error::Error + Send + 'static>(error: E) -> Self {
        SampleChannelError::Other(Box::new(error))
    }
}

#[atomic_enum::atomic_enum]
#[derive(PartialEq, Eq)]
pub enum SampleChannelStatus {
    NotStarted,
    Finished,
    Playing,
}

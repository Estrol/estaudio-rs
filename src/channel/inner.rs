use super::AudioChannelDSPCallback;
use crate::{
    channel::AudioChannelError,
    device::audioreader::AudioReader,
    effects::{
        AudioFX, AudioPanner, AudioResampler, AudioSpatialization, AudioSpatializationListener,
        AudioVolume,
    },
    utils,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};

#[allow(dead_code)]
pub(crate) struct AudioChannelInner {
    pub ref_id: usize,
    pub marked_as_deleted: bool,

    pub reader: AudioReader,
    pub last_time: Instant,

    pub gainer: AudioVolume,
    pub panner: AudioPanner,
    pub resampler: AudioResampler,
    pub fx: Option<AudioFX>,

    pub playing: Arc<AtomicBool>,
    pub is_looping: Arc<AtomicBool>,
    pub position: Arc<AtomicU64>,

    pub spatializer: Option<AudioSpatialization>,

    pub dsp_callback: Option<AudioChannelDSPCallback>,
    // pub slider: Vec<AudioSliderInstance>,
    pub start: Option<u64>,
    pub end: Option<u64>,
}

#[allow(clippy::undocumented_unsafe_blocks)]
unsafe impl Send for AudioChannelInner {}
#[allow(clippy::undocumented_unsafe_blocks)]
unsafe impl Sync for AudioChannelInner {}

#[allow(dead_code)]
impl AudioChannelInner {
    pub fn read_pcm_frames(
        &mut self,
        spatializer_listener: Option<&mut AudioSpatializationListener>,
        output: &mut [f32],
        temp: &mut [f32],
        frame_count: u64,
    ) -> Result<u64, AudioChannelError> {
        if !self.playing.load(Ordering::SeqCst) {
            return Ok(0);
        }

        let required_frame_count = self.resampler.get_required_input(frame_count).unwrap_or(0);

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
                target_frame_count = self
                    .reader
                    .read(output, target_frame_count)
                    .map_err(|e| AudioChannelError::AudioReaderError(e))?;

                if target_frame_count >= available_frames {
                    fx.frame_available += fx.get_output_latency() as i64;
                } else {
                    fx.frame_available += readed_frame_count as i64;
                }
            }

            if fx.frame_available > 0 {
                fx.process(output, target_frame_count, temp, readed_frame_count)
                    .map_err(|e| AudioChannelError::AudioFXError(e))?;

                fx.frame_available -= readed_frame_count as i64;

                if fx.frame_available < 0 {
                    readed_frame_count = (readed_frame_count as i64 + fx.frame_available) as u64;
                    fx.frame_available = 0;
                }
            } else {
                readed_frame_count = 0;
            }

            utils::array_fast_copy_f32(
                temp,
                output,
                0,
                0,
                (readed_frame_count * self.reader.channels as u64) as usize,
            );

            frames_readed = readed_frame_count;
        } else {
            frames_readed = self
                .reader
                .read(output, required_frame_count)
                .map_err(|e| AudioChannelError::AudioReaderError(e))?;
        }

        if !self.resampler.bypass_mode() {
            let resampler_frame_count = self
                .resampler
                .process(output, required_frame_count, temp, frame_count)
                .map_err(|e| AudioChannelError::AudioResamplerError(e))?;

            utils::array_fast_copy_f32(
                temp,
                output,
                0,
                0,
                (resampler_frame_count * self.reader.channels as u64) as usize,
            );

            frames_readed = resampler_frame_count;
        }

        self.gainer
            .process(output, temp, frames_readed as u64)
            .map_err(|e| AudioChannelError::AudioVolumeError(e))?;

        self.panner
            .process(temp, output, frames_readed as u64)
            .map_err(|e| AudioChannelError::AudioPannerError(e))?;

        self.position.fetch_add(frames_readed, Ordering::SeqCst);

        if frames_readed < frame_count {
            if self.is_looping.load(Ordering::SeqCst) {
                self.reader
                    .seek(0)
                    .map_err(|e| AudioChannelError::AudioReaderError(e))?;
            } else {
                self.playing.store(false, Ordering::SeqCst);
            }
        }

        if self.dsp_callback.is_some() {
            let callback = self.dsp_callback.as_ref().unwrap();
            callback(output, frames_readed);
        }

        if let Some(spatializer) = &mut self.spatializer {
            if let Some(listener) = spatializer_listener {
                spatializer
                    .process(listener, output, temp, frames_readed)
                    .map_err(|e| AudioChannelError::AudioSpatializationError(e))?;

                utils::array_fast_copy_f32(
                    temp,
                    output,
                    0,
                    0,
                    (frames_readed * self.reader.channels as u64) as usize,
                );
            }
        }

        return Ok(frames_readed);
    }

    pub fn seek(&mut self, position: u64) -> Result<u64, AudioChannelError> {
        if position >= self.reader.pcm_length {
            return Err(AudioChannelError::SeekOutOfBounds);
        }

        self.position.store(position, Ordering::SeqCst);

        self.reader
            .seek(position)
            .map_err(|e| AudioChannelError::AudioReaderError(e))?;

        if self.fx.is_some() {
            let fx = self.fx.as_mut().unwrap();
            let latency = fx.get_input_latency() as u64;

            // Only seek when need to feed the fx
            if latency > 0 {
                let mut output = vec![0.0f32; latency as usize * 2];

                self.reader
                    .read(&mut output, latency)
                    .map_err(|e| AudioChannelError::AudioReaderError(e))?;

                fx.pre_process(&output, latency)
                    .map_err(|e| AudioChannelError::AudioFXError(e))?;
            }
        }

        Ok(position)
    }

    pub fn is_playing(&self) -> bool {
        self.playing.load(Ordering::SeqCst)
    }
}

impl Drop for AudioChannelInner {
    fn drop(&mut self) {
        self.playing.store(false, Ordering::SeqCst);
    }
}

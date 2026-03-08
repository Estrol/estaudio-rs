use crate::{
    BufferInfo, SampleError, SampleInfo, TrackError, TrackInfo,
    audioreader::AudioReader,
    effects::{AudioPanner, AudioVolume, Resampler},
    misc::{
        audioattributes::AudioAttributes,
        audiopropertyhandler::{PropertyError, PropertyHandler},
    },
    sample::Sample,
    track::Track,
};

use astretch::Stretch;
use thiserror::Error;

pub mod writer;

#[derive(Debug, Default)]
pub struct EncoderInfo<'a> {
    pub source: crate::Source<'a>,
}

#[derive(Debug, Clone, Default)]
pub struct EncoderTrackInfo {
    pub channel: Option<usize>,
    pub sample_rate: Option<f32>,
}

#[derive(Debug, Clone, Default)]
pub struct EncoderSampleInfo {
    pub channel: Option<usize>,
    pub sample_rate: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct Encoder {
    reader: AudioReader,
    dirty: bool,

    fx: Stretch<f32>,
    resampler: Resampler,
    panner: AudioPanner,
    volume: AudioVolume,

    output: Vec<f32>,
    channel_count: usize,
    pcm_length: usize,
    sample_rate: f32,
    fx_rate: f32,
    fx_pitch: f32,
}

impl Encoder {
    pub(crate) fn new(info: EncoderInfo) -> Result<Self, EncoderError> {
        let (cache, buffer) = info.source.into_buffer();

        match (cache, buffer) {
            (Some(cache_key), _) => {
                let reader =
                    crate::macros::check!(AudioReader::load_cache(cache_key), EncoderError::InitFailed);
                Self::create_common(reader)
            }
            (_, Some(buffer)) => {
                let reader = crate::macros::check!(
                    AudioReader::load_audio_buffer(
                        buffer.data,
                        buffer.sample_rate,
                        buffer.channels,
                        buffer.data.len() / buffer.channels,
                        true
                    ),
                    EncoderError::InitFailed
                );
                Self::create_common(reader)
            }
            _ => Err(EncoderError::MissingSource),
        }
    }

    pub(crate) fn create_common(reader: AudioReader) -> Result<Self, EncoderError> {
        let resampler = crate::macros::check!(
            Resampler::new(reader.channels, reader.sample_rate),
            EncoderError::InitFailed
        );
        let panner = crate::macros::check!(AudioPanner::new(reader.channels), EncoderError::InitFailed);
        let volume = crate::macros::check!(AudioVolume::new(reader.channels), EncoderError::InitFailed);

        let mut fx = Stretch::<f32>::new();
        fx.preset_default(reader.channels as i32, reader.sample_rate as f32, true);

        let channel_count = reader.channels;
        let sample_rate = reader.sample_rate;

        Ok(Self {
            reader,
            dirty: true,
            fx,
            resampler,
            panner,
            volume,
            output: vec![],
            channel_count,
            pcm_length: 0,
            sample_rate,
            fx_pitch: 1.0,
            fx_rate: 1.0,
        })
    }

    pub(crate) fn encode(&mut self) -> Result<(), EncoderError> {
        if !self.dirty {
            return Ok(());
        }

        let mut samples =
            vec![0.0f32; self.reader.pcm_length as usize * self.reader.channels as usize];

        let result = self.reader.read(crate::macros::make_slice_mut!(
            samples,
            self.reader.pcm_length,
            self.reader.channels
        ));

        if let Err(e) = result {
            return Err(EncoderError::from_other(e));
        }

        let mut total_frame_count = self.reader.pcm_length;

        if self.fx_pitch != 1.0 || self.fx_rate != 1.0 {
            // HACK: This allow processing smaller audio files.
            const PRESETS: [(f32, f32); 3] = [
                (0.01f32, 0.004f32), // Slightly worse than presetDefault
                (0.001f32, 0.0004f32),
                (0.0001f32, 0.00004f32),
            ];

            let mut seek_length = self.fx.seek_length();
            if total_frame_count < seek_length {
                let sample_rate = self.reader.sample_rate;

                for (block, interval) in PRESETS {
                    if total_frame_count >= seek_length { 
                        break;
                    }

                    self.fx.configure(
                        self.reader.channels as i32,
                        (sample_rate * block) as i32,
                        (sample_rate * interval) as i32,
                        true
                    );

                    seek_length = self.fx.seek_length();
                }

                if total_frame_count < seek_length {
                    // If the audio is too short for the smallest preset, we can't process it with the current settings.
                    return Err(EncoderError::AudioEncoderError);
                }
            }

            self.fx
                .set_transpose_factor(self.fx_pitch, Some(8000.0 / self.sample_rate as f32));

            let output_count = (total_frame_count as f32 / self.fx_rate) as usize;
            let mut fx_output = vec![0.0f32; output_count * self.reader.channels];

            self.fx.exact(&samples, &mut fx_output);

            samples = fx_output;
            total_frame_count = output_count;
        }

        if !self.resampler.bypass_mode() {
            let expected_output_size = self.resampler.get_expected_output(total_frame_count);
            if let Err(e) = expected_output_size {
                return Err(EncoderError::from_other(e));
            }

            let expected_output_size = expected_output_size.unwrap();
            let mut resample_output =
                vec![0.0f32; expected_output_size as usize * self.reader.channels as usize];

            let Ok(size) = self.resampler.process(
                crate::macros::make_slice!(samples, total_frame_count, self.reader.channels),
                crate::macros::make_slice_mut!(
                    resample_output,
                    expected_output_size,
                    self.reader.channels
                ),
            ) else {
                return Err(EncoderError::from_other(result.err().unwrap()));
            };

            total_frame_count = size;
        }

        let mut buffer1 = vec![0.0f32; total_frame_count as usize * self.reader.channels as usize];

        let result = self.volume.process(&samples, &mut buffer1);

        if let Err(e) = result {
            return Err(EncoderError::from_other(e));
        }

        let result = self.panner.process(&buffer1, &mut samples);

        if let Err(e) = result {
            return Err(EncoderError::from_other(e));
        }

        self.output = samples;
        self.pcm_length = total_frame_count;
        self.dirty = false;

        Ok(())
    }

    pub fn get_data(&mut self) -> Result<&[f32], EncoderError> {
        self.encode()?;
        Ok(&self.output)
    }

    pub fn get_sample_rate(&self) -> f32 {
        self.sample_rate
    }

    pub fn get_channel_count(&self) -> usize {
        self.channel_count
    }

    pub fn save_as(&mut self, path: &str, format: writer::WriteFormat) -> Result<(), EncoderError> {
        let mut writer = crate::macros::check!(
            writer::Writer::new(path, format, self.channel_count, self.sample_rate),
            EncoderError::InitFailed
        );

        let data = self.get_data()?;
        writer
            .write(data)
            .map_err(|e| EncoderError::from_other(e))?;

        Ok(())
    }

    pub fn create_sample(
        &mut self,
        info: Option<EncoderSampleInfo>,
    ) -> Result<Sample, SampleError> {
        let sample_rate = self.get_sample_rate();
        let channels = self.get_channel_count();
        let data = self.get_data().map_err(|e| SampleError::from_other(e))?;

        let (sample_rate_info, channel_info) = if let Some(info) = info {
            (info.sample_rate, info.channel)
        } else {
            (None, None)
        };

        Sample::new(SampleInfo {
            source: crate::Source::Buffer(BufferInfo {
                data,
                channels,
                sample_rate: sample_rate as f32,
            }),
            sample_rate: sample_rate_info,
            channels: channel_info,
            ..Default::default()
        })
    }

    pub fn create_track(&mut self, info: Option<EncoderTrackInfo>) -> Result<Track, TrackError> {
        let sample_rate = self.get_sample_rate();
        let channels = self.get_channel_count();
        let data = self.get_data().map_err(|e| TrackError::from_other(e))?;

        let (channel_info, sample_rate_info) = if let Some(info) = info {
            (info.channel, info.sample_rate)
        } else {
            (None, None)
        };

        Track::new(TrackInfo {
            source: crate::Source::Buffer(BufferInfo {
                data,
                channels,
                sample_rate: sample_rate as f32,
            }),
            channel: channel_info,
            sample_rate: sample_rate_info,
            ..Default::default()
        })
    }
}

impl PropertyHandler for Encoder {
    fn get_attribute_f32(&self, _type: AudioAttributes) -> Result<f32, PropertyError> {
        match _type {
            AudioAttributes::FXTempo => Ok(self.fx_rate),
            AudioAttributes::FXPitch => Ok(self.fx_pitch),
            AudioAttributes::Pan => Ok(self.panner.pan),
            AudioAttributes::Volume => Ok(self.volume.volume),
            AudioAttributes::SampleRate => Ok(self.resampler.target_sample_rate as f32),
            _ => Err(PropertyError::NotImplemented),
        }
    }

    fn set_attribute_f32(
        &mut self,
        _type: AudioAttributes,
        _value: f32,
    ) -> Result<(), PropertyError> {
        match _type {
            AudioAttributes::FXTempo => {
                self.fx_rate = _value;
                self.dirty = true;
                Ok(())
            }
            AudioAttributes::FXPitch => {
                self.fx_pitch = _value;
                self.dirty = true;
                Ok(())
            }
            AudioAttributes::Pan => {
                self.panner.set_pan(_value);
                self.dirty = true;
                Ok(())
            }
            AudioAttributes::Volume => {
                self.volume.set_volume(_value);
                self.dirty = true;
                Ok(())
            }
            AudioAttributes::SampleRate => {
                self.resampler.set_target_sample_rate(_value);
                self.dirty = true;
                Ok(())
            }
            _ => Err(PropertyError::NotImplemented),
        }
    }
}

#[derive(Debug, Error)]
pub enum EncoderError {
    #[error("Missing audio source")]
    MissingSource,
    #[error("File not found: {0}")]
    FileNotFound(String),
    #[error("Unsupported audio format")]
    AudioEncoderError,
    #[error("Failed to initialize encoder")]
    InitFailed,
    #[error("{0}")]
    Other(Box<dyn std::error::Error + Send + 'static>),
}

impl EncoderError {
    pub fn from_other<E: std::error::Error + Send + 'static>(error: E) -> Self {
        EncoderError::Other(Box::new(error))
    }
}

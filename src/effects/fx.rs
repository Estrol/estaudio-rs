use astretch::Stretch;
use thiserror::Error;

#[derive(Debug)]
pub struct AudioFX {
    pub stretch: Stretch<f32>,
    pub channels: usize,
    pub sample_rate: f32,
    pub frame_available: isize,

    pub tempo: f32,
    pub octave: f32,
}

#[allow(dead_code)]
impl AudioFX {
    pub fn new(channels: usize, sample_rate: f32) -> Result<Self, AudioFXError> {
        if channels < 1 || channels > 8 {
            return Err(AudioFXError::InvalidConfiguration);
        }

        if sample_rate < 8000.0 || sample_rate > 192000.0 {
            return Err(AudioFXError::InvalidConfiguration);
        }

        let stretch = Stretch::new();

        Ok(Self {
            stretch,
            channels,
            sample_rate,
            frame_available: 0,
            tempo: 1.0,
            octave: 1.0,
        })
    }

    pub fn configure(&mut self, total_frame_count: usize) -> Result<usize, AudioFXError> {
        if total_frame_count == 0 {
            return Err(AudioFXError::InvalidConfiguration);
        }

        self.stretch.configure(
            self.channels as i32,
            self.sample_rate as i32,
            self.sample_rate as i32,
            true
        );

        // HACK: See (encoder/mod.rs#L130)
        const PRESETS: [(f32, f32); 3] = [
            (0.01f32, 0.004f32),
            (0.001f32, 0.0004f32),
            (0.0001f32, 0.00004f32),
        ];

        let mut seek_length = self.stretch.seek_length();
        if total_frame_count < seek_length {
            let sample_rate = self.sample_rate;

            for (block, interval) in PRESETS {
                if total_frame_count >= seek_length { 
                    break;
                }

                self.stretch.configure(
                    self.channels as i32,
                    (sample_rate * block) as i32,
                    (sample_rate * interval) as i32,
                    true
                );

                seek_length = self.stretch.seek_length();
            }

            if total_frame_count < seek_length {
                return Err(AudioFXError::InsufficientFrames);
            }
        }

        Ok(seek_length as usize)
    }

    pub fn get_input_latency(&self) -> usize {
        self.stretch.input_latency() as usize
    }

    pub fn get_output_latency(&self) -> usize {
        self.stretch.output_latency() as usize
    }

    pub fn get_seek_length(&self) -> usize {
        self.stretch.output_seek_length(self.tempo) as usize
    }

    pub fn get_required_input(&self, output_frame_count: usize) -> Result<usize, AudioFXError> {
        if output_frame_count == 0 {
            return Err(AudioFXError::InvalidFrameCount);
        }

        if self.tempo_bypass() {
            return Ok(output_frame_count);
        }

        let required_input = (output_frame_count as f32 * self.tempo).round() as usize;

        Ok(required_input)
    }

    pub fn get_expected_output(&self, input_frame_count: usize) -> Result<usize, AudioFXError> {
        if input_frame_count == 0 {
            return Err(AudioFXError::InvalidFrameCount);
        }

        if self.tempo_bypass() {
            return Ok(input_frame_count);
        }

        let output_frame_count = (input_frame_count as f32 / self.tempo).round() as usize;
        if output_frame_count == 0 {
            return Err(AudioFXError::InvalidFrameCount);
        }

        Ok(output_frame_count)
    }

    pub fn set_octave(&mut self, octave: f32) -> Result<(), AudioFXError> {
        if octave < 0.5 {
            return Err(AudioFXError::InvalidOctave);
        }

        let tonacity_limit = 4000.0 / self.sample_rate as f32;

        self.stretch
            .set_transpose_factor(octave, Some(tonacity_limit));

        self.octave = octave;

        Ok(())
    }

    pub fn set_tempo(&mut self, tempo: f32) -> Result<(), AudioFXError> {
        if tempo < 0.5 {
            return Err(AudioFXError::InvalidTempo);
        }

        if tempo > 2.0 {
            return Err(AudioFXError::InvalidTempo);
        }

        self.tempo = tempo;
        Ok(())
    }

    pub fn tempo_bypass(&self) -> bool {
        self.tempo == 1.0
    }

    pub fn seek(&mut self, input: &[f32]) -> Result<(), AudioFXError> {
        self.stretch.output_seek(&input);

        Ok(())
    }

    pub fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<(), AudioFXError> {
        let Ok(output_size) = self.get_expected_output(input.len() / self.channels as usize) else {
            return Err(AudioFXError::InvalidFrameCount);
        };

        let expected_output_size = output_size as usize * self.channels as usize;
        if output.len() < expected_output_size {
            return Err(AudioFXError::InvalidFrameCount);
        }

        let output = crate::macros::make_slice_mut!(output, output_size, self.channels);

        self.stretch.process(input, output);

        Ok(())
    }
}

#[derive(Debug, Error)]
#[must_use]
pub enum AudioFXError {
    #[error("AudioFX is not enabled. Please enable it before using.")]
    NotEnabled,
    #[error("Invalid configuration for AudioFX. Please check the parameters.")]
    InvalidConfiguration,
    #[error("Invalid frame count. Frame count must be greater than 0.")]
    InvalidFrameCount,
    #[error("Invalid tempo. Tempo must be greater than 0.5 and less than 2.0.")]
    InvalidTempo,
    #[error("Invalid octave. Octave must be greater than 0.5")]
    InvalidOctave,
    #[error("Insufficient required frames, make sure audio has enough frames for the current tempo setting, tried 3 presets but still not enough frames.")]
    InsufficientFrames,
}

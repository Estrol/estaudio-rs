use signalsmith_stretch::Stretch;

#[derive(Debug)]
pub struct AudioFX {
    pub stretch: Stretch,
    pub channels: u32,
    pub sample_rate: u32,
    pub frame_available: i64,

    pub tempo: f32,
    pub octave: f32,
}

#[allow(dead_code)]
impl AudioFX {
    pub fn new(channels: u32, sample_rate: u32) -> Result<Self, AudioFXError> {
        if channels < 1 || channels > 8 {
            return Err(AudioFXError::InvalidConfiguration);
        }

        if sample_rate < 8000 || sample_rate > 192000 {
            return Err(AudioFXError::InvalidConfiguration);
        }

        let stretch = Stretch::preset_default(channels, sample_rate);

        Ok(Self {
            stretch,
            channels,
            sample_rate,
            frame_available: 0,
            tempo: 1.0,
            octave: 1.0,
        })
    }

    pub fn get_input_latency(&self) -> u32 {
        self.stretch.input_latency() as u32
    }

    pub fn get_output_latency(&self) -> u32 {
        self.stretch.output_latency() as u32
    }

    pub fn get_required_input(&self, output_frame_count: u64) -> Result<u64, AudioFXError> {
        if output_frame_count == 0 {
            return Err(AudioFXError::InvalidFrameCount);
        }

        if self.tempo_bypass() {
            return Ok(output_frame_count);
        }

        let required_input = (output_frame_count as f32 * self.tempo) as u64;

        Ok(required_input)
    }

    pub fn get_expected_output(&self, input_frame_count: u64) -> Result<u64, AudioFXError> {
        if input_frame_count == 0 {
            return Err(AudioFXError::InvalidFrameCount);
        }

        if self.tempo_bypass() {
            return Ok(input_frame_count);
        }

        let output_frame_count = (input_frame_count as f32 / self.tempo) as u64;
        if output_frame_count == 0 {
            return Err(AudioFXError::InvalidFrameCount);
        }

        Ok(output_frame_count)
    }

    pub fn set_octave(&mut self, octave: f32) -> Result<(), AudioFXError> {
        let tonacity_limit = 4000.0 / self.sample_rate as f32;

        self.stretch
            .set_transpose_factor(octave, Some(tonacity_limit));

        self.octave = octave;

        Ok(())
    }

    pub fn set_tempo(&mut self, tempo: f32) -> Result<(), AudioFXError> {
        self.tempo = tempo;
        Ok(())
    }

    pub fn tempo_bypass(&self) -> bool {
        self.tempo == 1.0
    }

    pub fn pre_process(&mut self, input: &[f32], frame_count: u64) -> Result<(), AudioFXError> {
        if input.len() < (frame_count * self.channels as u64) as usize {
            return Err(AudioFXError::InvalidInputSize {
                expected: (frame_count * self.channels as u64) as usize,
                actual: input.len(),
            });
        }

        self.stretch.reset();
        self.stretch.process_raw(input, frame_count as usize, [], 0);

        self.frame_available = frame_count as i64;

        Ok(())
    }

    pub fn process(
        &mut self,
        input: &[f32],
        input_frame_count: u64,
        output: &mut [f32],
        output_frame_count: u64,
    ) -> Result<(), AudioFXError> {
        if input.len() < (input_frame_count * self.channels as u64) as usize {
            return Err(AudioFXError::InvalidInputSize {
                expected: (input_frame_count * self.channels as u64) as usize,
                actual: input.len(),
            });
        }

        if output.len() < (output_frame_count * self.channels as u64) as usize {
            return Err(AudioFXError::InvalidOutputSize {
                expected: (output_frame_count * self.channels as u64) as usize,
                actual: output.len(),
            });
        }

        self.stretch.process_raw(
            input,
            input_frame_count as usize,
            output,
            output_frame_count as usize,
        );

        Ok(())
    }
}

#[derive(Debug, Clone)]
#[must_use]
pub enum AudioFXError {
    NotEnabled,
    InvalidConfiguration,
    InvalidInputSize {
        expected: usize,
        actual: usize,
    },
    InvalidOutputSize {
        expected: usize,
        actual: usize,
    },
    BufferTooSmall {
        buffer: &'static str, // "input" or "output"
        expected: usize,
        actual: usize,
    },
    InvalidFrameCount,
    InvalidTempo,
    InvalidOctave,
    LibraryError(String),
    Other(String),
}

impl std::fmt::Display for AudioFXError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioFXError::NotEnabled => {
                write!(f, "AudioFX is not enabled. Please enable it before using.")
            }
            AudioFXError::InvalidConfiguration => {
                write!(
                    f,
                    "Invalid configuration for AudioFX. Please check the parameters."
                )
            }
            AudioFXError::InvalidInputSize { expected, actual } => {
                write!(
                    f,
                    "Input buffer size does not match the expected size. Expected: {}, Got: {}",
                    expected, actual
                )
            }

            AudioFXError::InvalidOutputSize { expected, actual } => {
                write!(
                    f,
                    "Output buffer size does not match the expected size. Expected: {}, Got: {}",
                    expected, actual
                )
            }

            AudioFXError::InvalidFrameCount => {
                write!(
                    f,
                    "Invalid frame count. Frame count must be greater than 0."
                )
            }

            AudioFXError::BufferTooSmall {
                buffer,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "Buffer '{}' is too small. Expected: {}, Got: {}",
                    buffer, expected, actual
                )
            }

            AudioFXError::InvalidTempo => {
                write!(f, "Invalid tempo. Tempo must be greater than 0.")
            }

            AudioFXError::InvalidOctave => {
                write!(f, "Invalid octave. Octave must be greater than 0.")
            }

            AudioFXError::LibraryError(msg) => {
                write!(f, "Library error: {}", msg)
            }

            AudioFXError::Other(msg) => {
                write!(f, "An error occurred: {}", msg)
            }
        }
    }
}

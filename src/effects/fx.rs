use signalsmith_stretch::Stretch;

pub struct AudioFX {
    pub stretch: Stretch,
    pub channels: u32,
    pub sample_rate: u32,
    pub frame_available: i64,

    pub tempo: f32,
    pub octave: f32,
}

impl AudioFX {
    pub fn new(channels: u32, sample_rate: u32) -> Result<Self, String> {
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

    pub fn get_required_input(&self, output_frame_count: u64) -> Result<u64, String> {
        if output_frame_count == 0 {
            return Err("Output frame count cannot be zero.".to_string());
        }

        if self.tempo_bypass() {
            return Ok(output_frame_count);
        }

        let required_input = (output_frame_count as f32 * self.tempo) as u64;

        Ok(required_input)
    }

    pub fn get_expected_output(&self, input_frame_count: u64) -> Result<u64, String> {
        if input_frame_count == 0 {
            return Err("Input frame count cannot be zero.".to_string());
        }

        if self.tempo_bypass() {
            return Ok(input_frame_count);
        }

        let output_frame_count = (input_frame_count as f32 / self.tempo) as u64;
        if output_frame_count == 0 {
            return Err("Output frame count cannot be zero.".to_string());
        }

        Ok(output_frame_count)
    }

    pub fn set_octave(&mut self, octave: f32) -> Result<(), String> {
        self.stretch
            .set_transpose_factor(octave, Some(4000.0 / self.sample_rate as f32));
        self.octave = octave;
        Ok(())
    }

    pub fn set_tempo(&mut self, tempo: f32) -> Result<(), String> {
        self.tempo = tempo;
        Ok(())
    }

    pub fn tempo_bypass(&self) -> bool {
        self.tempo == 1.0
    }

    pub fn pre_process(&mut self, input: &[f32], frame_count: u64) -> Result<(), String> {
        if input.len() < (frame_count * self.channels as u64) as usize {
            return Err(format!(
                "Input buffer size does not match the expected size. Expected: {}, Got: {}",
                frame_count * self.channels as u64,
                input.len()
            ));
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
    ) -> Result<(), String> {
        if input.len() < (input_frame_count * self.channels as u64) as usize {
            return Err(format!(
                "Input buffer size does not match the expected size. Expected: {}, Got: {}",
                input_frame_count * self.channels as u64,
                input.len()
            ));
        }

        if output.len() < (output_frame_count * self.channels as u64) as usize {
            return Err(format!(
                "Output buffer size does not match the expected size. Expected: {}, Got: {}",
                output_frame_count * self.channels as u64,
                output.len()
            ));
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

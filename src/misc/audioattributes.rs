#[repr(C)]
pub enum AudioAttributes {
    Unknown,
    /// The sample rate of the audio channel, device or mixer.
    SampleRate,
    /// The volume of the audio channel, device or mixer.
    Volume,
    /// The pan of the audio channel, device or mixer.
    Pan,
    /// The pitch of the audio channel. \
    /// This require the [AudioAttributes::FXEnabled] on [AudioDevice] to be enabled.
    FXPitch,
    /// The tempo of the audio channel. \
    /// This require the [AudioAttributes::FXEnabled] on [AudioDevice] to be enabled.
    FXTempo,
    /// Enable or disable the AudioFX used for Tempo and Pitch on the audio channel, device or mixer.
    FXEnabled,
    /// Enable or disable the AudioSpatialization used for 3D Audio on the audio channel, device or mixer.
    SpatializationEnabled,
}

impl AudioAttributes {
    pub fn from(name: &str) -> Self {
        match name {
            "SampleRate" => AudioAttributes::SampleRate,
            "Volume" => AudioAttributes::Volume,
            "Pan" => AudioAttributes::Pan,
            "FXPitch" => AudioAttributes::FXPitch,
            "FXTempo" => AudioAttributes::FXTempo,
            _ => AudioAttributes::Unknown,
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            AudioAttributes::SampleRate => "SampleRate".to_string(),
            AudioAttributes::Volume => "Volume".to_string(),
            AudioAttributes::Pan => "Pan".to_string(),
            AudioAttributes::FXPitch => "FXPitch".to_string(),
            AudioAttributes::FXTempo => "FXTempo".to_string(),
            AudioAttributes::FXEnabled => "FXEnabled".to_string(),
            AudioAttributes::SpatializationEnabled => "AudioSpatialization".to_string(),
            AudioAttributes::Unknown => "Unknown".to_string(),
        }
    }
}

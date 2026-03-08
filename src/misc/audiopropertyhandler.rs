use thiserror::Error;

use super::audioattributes::AudioAttributes;

pub trait PropertyHandler {
    /// Get the [AudioAttributes] value (f32) of the [AudioChannel], [AudioDevice] or [AudioMixer].
    fn get_attribute_f32(&self, _type: AudioAttributes) -> Result<f32, PropertyError> {
        Err(PropertyError::NotImplemented)
    }
    /// Set the [AudioAttributes] value (f32) of the [AudioChannel], [AudioDevice] or [AudioMixer].
    fn set_attribute_f32(
        &mut self,
        _type: AudioAttributes,
        _value: f32,
    ) -> Result<(), PropertyError> {
        Err(PropertyError::NotImplemented)
    }
    /// Get the [AudioAttributes] value (bool) of the [AudioChannel], [AudioDevice] or [AudioMixer].
    fn get_attribute_bool(&self, _type: AudioAttributes) -> Result<bool, PropertyError> {
        Err(PropertyError::NotImplemented)
    }
    /// Set the [AudioAttributes] value (bool) of the [AudioChannel], [AudioDevice] or [AudioMixer].
    fn set_attribute_bool(
        &mut self,
        _type: AudioAttributes,
        _value: bool,
    ) -> Result<(), PropertyError> {
        Err(PropertyError::NotImplemented)
    }
}

#[derive(Debug, Error)]
pub enum PropertyError {
    #[error("Attribute not implemented")]
    NotImplemented,
    #[error("Unsupported attribute: {0}")]
    UnsupportedAttribute(&'static str),
    #[error("Invalid operation: {0}")]
    InvalidOperation(&'static str),
    #[error("{0}")]
    Other(Box<dyn std::error::Error + 'static>),
}

impl PropertyError {
    pub fn from_other<E: std::error::Error + 'static>(error: E) -> Self {
        PropertyError::Other(Box::new(error))
    }
}

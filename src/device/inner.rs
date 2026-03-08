use miniaudio_sys::*;
use std::sync::{Arc, TryLockError, mpsc::Receiver};

use crate::{
    DeviceInfo,
    context::{DeviceType, MaContext},
    device::{AudioHandle, DeviceError},
    effects::{AudioPanner, SpatializationListener, AudioVolume, ChannelConverter},
    math::{MathUtils, MathUtilsTrait as _},
};

pub struct TrackChannelHandle {
    pub channel: AudioHandle,
    pub removed: bool,
}

pub(crate) struct DeviceInner {
    pub context: Option<Arc<MaContext>>,
    pub device: Box<ma_device>,
    pub ty: DeviceType,

    pub handles: Vec<TrackChannelHandle>,
    pub volume: AudioVolume,
    pub panner: AudioPanner,
    pub channel_converter: ChannelConverter,
    pub buffer1: Vec<f32>,
    pub buffer2: Vec<f32>,

    // DSP callback
    pub callback: Option<Box<dyn FnMut(&[f32], &mut [f32]) + Send + 'static>>,
    pub input_callback: Option<Box<dyn FnMut(&[f32]) + Send + 'static>>,
    pub output_callback: Option<Box<dyn FnMut(&mut [f32]) + Send + 'static>>,

    // Spatialization
    pub spatialization: Option<SpatializationListener>,

    pub receiver: Receiver<AudioHandle>,
}

impl DeviceInner {
    pub fn new(
        config: DeviceInfo,
    ) -> Result<(Box<Self>, std::sync::mpsc::Sender<AudioHandle>), DeviceError> {
        unsafe {
            let (sender, receiver) = std::sync::mpsc::channel();

            let channel_count = config.channel;
            let sample_rate = config.sample_rate;
            let device_type = config.ty;

            let mut inner = Box::new(Self {
                context: None,
                device: Box::default(),
                handles: Vec::new(),
                ty: device_type,
                buffer1: vec![0.0f32; 4096 * channel_count],
                buffer2: vec![0.0f32; 4096 * channel_count],
                spatialization: None,
                volume: AudioVolume::new(channel_count).map_err(DeviceError::from_other)?,
                panner: AudioPanner::new(channel_count).map_err(DeviceError::from_other)?,
                channel_converter: ChannelConverter::new(),
                callback: None,
                input_callback: None,
                output_callback: None,
                receiver,
            });

            let device_type = match config.ty {
                DeviceType::Playback => ma_device_type_playback,
                DeviceType::Capture => ma_device_type_capture,
                DeviceType::Duplex => ma_device_type_duplex,
            };

            let mut devconfig = ma_device_config_init(device_type);

            devconfig.playback.format = ma_format_f32;
            devconfig.playback.channels = channel_count as u32;
            devconfig.sampleRate = sample_rate as u32;
            devconfig.dataCallback = Some(audio_callback);
            devconfig.pUserData = inner.as_mut() as *mut _ as *mut std::ffi::c_void;
            devconfig.noClip = MA_TRUE as u8; // We use SIMD clamping
            devconfig.noPreSilencedOutputBuffer = MA_TRUE as u8; // We use SIMD zeroing

            // Store temporary context for lifetime and validation purposes.
            let mut context = None;
            match config.ty {
                DeviceType::Playback => {
                    if let Some(hw_info) = config.output {
                        if hw_info.ty != DeviceType::Playback {
                            return Err(DeviceError::UnsupportedHardwareDevice);
                        }

                        if hw_info.id.is_some() {
                            devconfig.playback.pDeviceID = hw_info.id.as_ref().unwrap();
                        }

                        context = Some(Arc::clone(&hw_info.ctx));
                    }
                }
                DeviceType::Capture => {
                    if let Some(hw_info) = config.input {
                        if hw_info.ty != DeviceType::Capture {
                            return Err(DeviceError::UnsupportedHardwareDevice);
                        }

                        if hw_info.id.is_some() {
                            devconfig.capture.pDeviceID = hw_info.id.as_ref().unwrap();
                        }

                        context = Some(Arc::clone(&hw_info.ctx));
                    }
                }
                DeviceType::Duplex => {
                    if let Some(hw_info) = config.output {
                        if hw_info.ty != DeviceType::Playback {
                            return Err(DeviceError::UnsupportedHardwareDevice);
                        }

                        if hw_info.id.is_some() {
                            devconfig.playback.pDeviceID = hw_info.id.as_ref().unwrap();
                        }

                        context = Some(Arc::clone(&hw_info.ctx));
                    }

                    if let Some(hw_info) = config.input {
                        if hw_info.ty != DeviceType::Capture {
                            return Err(DeviceError::UnsupportedHardwareDevice);
                        }

                        if hw_info.id.is_some() {
                            devconfig.capture.pDeviceID = hw_info.id.as_ref().unwrap();
                        }

                        // Have to check if context is same.
                        if let Some(context) = &context {
                            if !Arc::ptr_eq(context, &hw_info.ctx) {
                                return Err(DeviceError::UnsupportedHardwareDevice);
                            }
                        } else {
                            context = Some(Arc::clone(&hw_info.ctx));
                        }
                    }
                }
            }

            let result = if let Some(context) = context {
                inner.context = Some(Arc::clone(&context));
                ma_device_init(context.as_mut_ptr(), &devconfig, inner.device.as_mut())
            } else {
                ma_device_init(std::ptr::null_mut(), &devconfig, inner.device.as_mut())
            };

            if result != MA_SUCCESS {
                return Err(DeviceError::InitializationError(result));
            }

            Ok((inner, sender))
        }
    }

    pub fn start(&mut self) -> Result<(), DeviceError> {
        unsafe {
            let result = ma_device_start(self.device.as_mut());
            if result != MA_SUCCESS {
                return Err(DeviceError::InitializationError(result));
            }
        }
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), DeviceError> {
        unsafe {
            let result = ma_device_stop(self.device.as_mut());
            if result != MA_SUCCESS {
                return Err(DeviceError::InitializationError(result));
            }
        }
        Ok(())
    }

    pub fn set_callback<F>(&mut self, callback: Option<F>) -> Result<(), DeviceError>
    where
        F: FnMut(&[f32], &mut [f32]) + Send + 'static,
    {
        self.callback =
            callback.map(|cb| Box::new(cb) as Box<dyn FnMut(&[f32], &mut [f32]) + Send + 'static>);
        Ok(())
    }

    pub fn set_input_callback<F>(&mut self, callback: Option<F>) -> Result<(), DeviceError>
    where
        F: FnMut(&[f32]) + Send + 'static,
    {
        self.input_callback =
            callback.map(|cb| Box::new(cb) as Box<dyn FnMut(&[f32]) + Send + 'static>);
        Ok(())
    }

    pub fn set_output_callback<F>(&mut self, callback: Option<F>) -> Result<(), DeviceError>
    where
        F: FnMut(&mut [f32]) + Send + 'static,
    {
        self.output_callback =
            callback.map(|cb| Box::new(cb) as Box<dyn FnMut(&mut [f32]) + Send + 'static>);
        Ok(())
    }

    pub fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
    ) -> Result<(), DeviceError> {
        MathUtils::simd_set(output, 0.0);
        MathUtils::simd_set(&mut self.buffer1, 0.0);
        MathUtils::simd_set(&mut self.buffer2, 0.0);

        let target_channel_count = self.device.playback.channels;

        while let Ok(handle) = self.receiver.try_recv() {
            self.handles.push(TrackChannelHandle {
                channel: handle,
                removed: false,
            });
        }

        if self.handles.is_empty() && self.callback.is_none() {
            return Ok(());
        }

        let frame_count =
            crate::macros::frame_count_from!(output.len(), target_channel_count as usize);

        for handle in self.handles.iter_mut() {
            if handle.removed {
                continue;
            }

            match &handle.channel {
                AudioHandle::Track(track_weak) => {
                    if let Some(track_mutex) = track_weak.upgrade() {
                        match track_mutex.try_lock() {
                            Ok(mut track) => {
                                match track.read(
                                    self.spatialization.as_mut(),
                                    &mut self.channel_converter,
                                    &mut self.buffer1,
                                    &mut self.buffer2,
                                    frame_count,
                                ) {
                                    Ok(pcm_length) => {
                                        if pcm_length > 0 {
                                            let size =
                                                pcm_length as usize * target_channel_count as usize;
                                            MathUtils::simd_add(
                                                &mut output[..size],
                                                &self.buffer1[..size],
                                            );
                                        } else {
                                            handle.removed = true;
                                        }
                                    }
                                    Err(err) => {
                                        eprintln!("Error reading PCM frames: {}", err);
                                        handle.removed = true;
                                    }
                                }
                            }
                            Err(TryLockError::Poisoned(channel)) => {
                                let ref_id = channel.get_ref().ref_id;

                                eprintln!("Warning: Audio channel {} is poisoned", ref_id);
                                handle.removed = true;
                            }
                            Err(TryLockError::WouldBlock) => {
                                continue;
                            }
                        }
                    } else {
                        handle.removed = true;
                    }
                }
                AudioHandle::Sample(sample_weak) => {
                    if let Some(sample_mutex) = sample_weak.upgrade() {
                        match sample_mutex.try_lock() {
                            Ok(mut sample) => {
                                match sample.read(
                                    self.spatialization.as_mut(),
                                    &mut self.channel_converter,
                                    &mut self.buffer1,
                                    &mut self.buffer2,
                                    frame_count,
                                ) {
                                    Ok(pcm_length) => {
                                        if pcm_length > 0 {
                                            let size =
                                                pcm_length as usize * target_channel_count as usize;
                                            MathUtils::simd_add(
                                                &mut output[..size],
                                                &self.buffer1[..size],
                                            );
                                        } else {
                                            handle.removed = true;
                                        }
                                    }
                                    Err(err) => {
                                        eprintln!("Error reading PCM frames from sample: {}", err);
                                        handle.removed = true;
                                    }
                                }
                            }
                            Err(TryLockError::Poisoned(sample)) => {
                                let ref_id = sample.get_ref().ref_id;

                                eprintln!("Warning: Sample channel {} is poisoned", ref_id);
                                handle.removed = true;
                            }
                            Err(TryLockError::WouldBlock) => {
                                continue;
                            }
                        }
                    } else {
                        handle.removed = true;
                    }
                }
                AudioHandle::Mixer(mixer_weak) => {
                    if let Some(mixer_mutex) = mixer_weak.upgrade() {
                        match mixer_mutex.try_lock() {
                            Ok(mut mixer) => {
                                match mixer.read(
                                    self.spatialization.as_mut(),
                                    &mut self.channel_converter,
                                    &mut self.buffer1,
                                    &mut self.buffer2,
                                    frame_count,
                                ) {
                                    Ok(pcm_length) => {
                                        if pcm_length > 0 {
                                            let size =
                                                pcm_length as usize * target_channel_count as usize;
                                            MathUtils::simd_add(
                                                &mut output[..size],
                                                &self.buffer1[..size],
                                            );
                                        } else {
                                            handle.removed = true;
                                        }
                                    }
                                    Err(err) => {
                                        eprintln!("Error reading PCM frames from mixer: {}", err);
                                        handle.removed = true;
                                    }
                                }
                            }
                            Err(TryLockError::Poisoned(mixer)) => {
                                let ref_id = mixer.get_ref().ref_id;

                                eprintln!("Warning: Mixer channel {} is poisoned", ref_id);
                                handle.removed = true;
                            }
                            Err(TryLockError::WouldBlock) => {
                                continue;
                            }
                        }
                    } else {
                        handle.removed = true;
                    }
                }
            }
        }

        if let Some(callback) = &mut self.callback {
            callback(input, output);
        }

        if let Some(input_callback) = &mut self.input_callback {
            input_callback(input);
        }

        if let Some(output_callback) = &mut self.output_callback {
            output_callback(output);
        }

        let buffer1 = crate::macros::make_slice_mut!(
            self.buffer1,
            frame_count,
            target_channel_count as usize
        );

        if let Err(e) = self.panner.process(output, buffer1) {
            eprintln!("Error processing panner: {}", e);
        }

        if let Err(e) = self.volume.process(buffer1, output) {
            eprintln!("Error processing volume: {}", e);
        }

        self.handles.retain(|ch| !ch.removed);
        MathUtils::simd_clamp(output, -1.0, 1.0);

        return Ok(());
    }
}

#[allow(non_snake_case)]
pub(crate) extern "C" fn audio_callback(
    _p: *mut ma_device,
    _pOutput: *mut std::ffi::c_void,
    _pInput: *const std::ffi::c_void,
    _frameCount: u32,
) {
    let result = std::panic::catch_unwind(|| {
        // SAFETY: All the pointers are valid and the function is called in a safe context.
        // The pointers were constructed by the miniaudio library and are valid for the duration of the callback
        // as long as the device is running and the array bounds within the frame count x channels are respected.
        unsafe {
            let device = &mut *_p;
            if device.pUserData.is_null() {
                return;
            }

            let inner = (device.pUserData as *mut DeviceInner)
                .as_mut()
                .unwrap();

            let channel_count = device.playback.channels as usize;

            let empty_input = [0f32; 0];
            let mut empty_output = [0f32; 0];

            let (input, output) = match inner.ty {
                DeviceType::Playback => {
                    let output = std::slice::from_raw_parts_mut(
                        _pOutput as *mut f32,
                        _frameCount as usize * channel_count,
                    );

                    (empty_input.as_slice(), output)
                }
                DeviceType::Capture => {
                    let input = std::slice::from_raw_parts(
                        _pInput as *mut f32,
                        _frameCount as usize * channel_count,
                    );

                    (input, empty_output.as_mut_slice())
                }
                DeviceType::Duplex => {
                    let input = std::slice::from_raw_parts(
                        _pInput as *mut f32,
                        _frameCount as usize * channel_count,
                    );

                    let output = std::slice::from_raw_parts_mut(
                        _pOutput as *mut f32,
                        _frameCount as usize * channel_count,
                    );

                    (input, output)
                }
            };

            inner.process(input, output).unwrap_or_else(|err| {
                eprintln!("Error processing audio: {}", err);
            });
        }
    });

    if let Err(err) = result {
        eprintln!("Rust panic! in audio callback: {:?}", err);
    }
}

impl Drop for DeviceInner {
    fn drop(&mut self) {
        _ = self.stop();

        // SAFETY: This function is safe because it properly uninitializes the audio device and decoders.
        // The code ensures that all resources are released and cleaned up.
        unsafe {
            self.handles.clear();
            ma_device_uninit(self.device.as_mut());
        }
    }
}

use est_audio::PropertyHandler;

extern crate est_audio;

pub fn main() {
    let config = est_audio::DeviceInfo {
        ty: est_audio::DeviceType::Duplex,
        channel: 2,
        sample_rate: 44100.0,
        ..Default::default()
    };

    let Ok(mut device) = est_audio::create_device(config) else {
        println!("Failed to create audio device");
        return;
    };

    device
        .set_callback(Some(|input: &[f32], output: &mut [f32]| {
            for (i, sample) in input.iter().enumerate() {
                output[i] += *sample;
            }
        }))
        .unwrap();

    device.start().unwrap();

    let config = est_audio::TrackInfo {
        source: est_audio::Source::Path("C:\\Users\\Estrol\\Downloads\\example3.wav"),
        ..Default::default()
    };

    let Ok(mut track) = est_audio::create_track(config) else {
        println!("Failed to create track");
        return;
    };

    track.set_attribute_bool(est_audio::AudioAttributes::FXEnabled, true).unwrap();
    track.set_attribute_f32(est_audio::AudioAttributes::FXTempo, 1.5).unwrap();

    track
        .play(&mut device)
        .unwrap();

    loop {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

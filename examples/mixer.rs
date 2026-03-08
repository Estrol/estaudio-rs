use est_audio::PropertyHandler as _;

extern crate est_audio;

pub fn main() {
    let config = est_audio::DeviceInfo {
        ty: est_audio::DeviceType::Playback,
        channel: 2,
        sample_rate: 44100.0,
        ..Default::default()
    };

    let Ok(mut device) = est_audio::create_device(config) else {
        println!("Failed to create audio device");
        return;
    };

    let config = est_audio::TrackInfo {
        source: est_audio::Source::Path("C:\\Users\\Estrol\\Downloads\\example3.wav"),
        ..Default::default()
    };

    let config2 = est_audio::TrackInfo {
        source: est_audio::Source::Path("C:\\Users\\Estrol\\Downloads\\example4.wav"),
        ..Default::default()
    };

    let Ok(track) = est_audio::create_track(config) else {
        println!("Failed to create track");
        return;
    };

    let Ok(track2) = est_audio::create_track(config2) else {
        println!("Failed to create track");
        return;
    };

    let mixer_config = est_audio::MixerInfo {
        channel: 2,
        sample_rate: 44100.0,
        tracks: vec![
            est_audio::MixerInput::Track(&track),
            est_audio::MixerInput::Track(&track2),
        ],
        ..Default::default()
    };

    let Ok(mut mixer) = est_audio::create_mixer(mixer_config) else {
        println!("Failed to create mixer");
        return;
    };

    mixer.add_track(&track).unwrap();
    mixer.add_track(&track2).unwrap();
    mixer.set_normalize_output(true).unwrap();

    mixer.set_attribute_bool(est_audio::AudioAttributes::FXEnabled, true).unwrap();
    mixer.set_attribute_f32(est_audio::AudioAttributes::FXTempo, 1.5).unwrap();
    mixer.play(&mut device).unwrap();

    loop {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
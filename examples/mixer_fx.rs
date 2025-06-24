extern crate est_audio;

use est_audio::prelude::*;

fn main() {
    let mut engine = est_audio::create_device(None)
        .build()
        .expect("Failed to create audio engine");

    let channel = est_audio::create_channel(None)
        .file("./assets/Example.ogg")
        .build()
        .expect("Failed to create audio channel");

    let mixer = est_audio::create_mixer(None)
        .channel(2)
        .sample_rate(44100)
        .build()
        .expect("Failed to create audio mixer");

    engine
        .add_mixer(&mixer)
        .expect("Failed to add mixer to engine");

    mixer
        .set_attribute_bool(AudioAttributes::AudioFX, true)
        .expect("Failed to set audio FX");
    mixer
        .set_attribute_f32(AudioAttributes::FXPitch, 1.25)
        .expect("Failed to set pitch");

    mixer
        .add_channel(&channel)
        .expect("Failed to add channel to mixer");

    mixer.play().expect("Failed to play mixer");

    while mixer.is_playing() {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

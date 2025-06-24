extern crate est_audio;

use est_audio::prelude::*;

fn main() {
    let mut engine = est_audio::create_device(None)
        .build()
        .expect("Failed to create audio engine");

    let mut channel = est_audio::create_channel(Some(&mut engine))
        .file("./assets/Example.ogg")
        .build()
        .expect("Failed to create audio channel");

    channel
        .set_attribute_bool(AudioAttributes::AudioFX, true)
        .expect("Failed to set audio FX");
    channel
        .set_attribute_f32(AudioAttributes::FXPitch, 1.25)
        .expect("Failed to set pitch");

    channel.play().expect("Failed to play audio channel");

    while channel.is_playing() {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

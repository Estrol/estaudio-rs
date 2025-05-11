extern crate estaudioengine;

use estaudioengine::prelude::*;

fn main() {
    let engine = AudioEngine::make_device(None)
        .build()
        .expect("Failed to create audio engine");

    let channel = AudioEngine::make_channel(None)
        .file("./assets/Example.ogg")
        .build()
        .expect("Failed to create audio channel");

    let mixer = AudioEngine::make_mixer()
        .channel(2)
        .sample_rate(44100)
        .build()
        .expect("Failed to create audio mixer");

    engine.add_mixer(&mixer)
        .expect("Failed to add mixer to engine");

    mixer.add_channel(&channel)
        .expect("Failed to add channel to mixer");

    mixer.play().expect("Failed to play mixer");

    while mixer.is_playing() {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
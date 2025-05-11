extern crate estaudioengine;

use estaudioengine::prelude::*;

fn main() {
    let engine = AudioEngine::make_device(None)
        .build()
        .expect("Failed to create audio engine");

    let channel = AudioEngine::make_channel(Some(&engine))
        .file("./assets/Example.ogg")
        .build()
        .expect("Failed to create audio channel");

    channel.play().expect("Failed to play audio channel");

    while channel.is_playing() {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
extern crate est_audio;

fn main() {
    let mut engine = est_audio::create_device(None)
        .build()
        .expect("Failed to create audio engine");

    let mut channel = est_audio::create_channel(Some(&mut engine))
        .file("./assets/Example.ogg")
        .build()
        .expect("Failed to create audio channel");

    channel.play().expect("Failed to play audio channel");

    while channel.is_playing() {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

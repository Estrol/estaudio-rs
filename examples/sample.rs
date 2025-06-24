extern crate est_audio;

fn main() {
    let engine = est_audio::create_device(None)
        .build()
        .expect("Failed to create audio engine");

    let sample = est_audio::create_sample()
        .file("./assets/Example.ogg")
        .build()
        .expect("Failed to create audio sample");

    let mut channels = sample
        .get_channels(&engine, 2)
        .expect("Failed to create audio channel");

    if channels.is_empty() {
        panic!("No channels found");
    }

    for channel in channels.iter_mut() {
        channel.play().expect("Failed to play audio channel");
        std::thread::sleep(std::time::Duration::from_secs(1));
    }

    // Wait for the channels to finish playing
    for channel in channels {
        while channel.is_playing() {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}

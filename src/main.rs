use estaudioengine::prelude::*;

fn main() {
    let device = AudioEngine::make_device(None)
        .build()
        .expect("Failed to create audio device");

    let channel = AudioEngine::make_channel(None)
        .file("D:\\ZombieZ_Start.ogg")
        .build()
        .expect("Failed to create audio channel");

    let channel2 = AudioEngine::make_channel(None)
        .file("D:\\ZombieZ_Start.ogg")
        .build()
        .expect("Failed to create audio channel");

    let mixer = AudioEngine::make_mixer()
        .device(&device)
        .channel(2)
        .build()
        .expect("Failed to create audio mixer");

    mixer.set_attribute_bool(AudioAttributes::AudioFX, true).expect("Failed to set audio attributes");
    mixer.set_attribute_f32(AudioAttributes::FXTempo, 1.5).expect("Failed to set audio attributes");

    mixer.add_channel(&channel)
        .expect("Failed to add channel to mixer");

    mixer
        .add_channel_ex(
            &channel2,
            PCMIndex::from_millis(1500.0, 44100),
            PCMIndex::from_millis(2500.0, 44100),
        )
        .expect("Failed to add channel to mixer");

    mixer.play().expect("Failed to play mixer");

    while mixer.is_playing() {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

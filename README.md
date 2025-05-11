# EstAudioEngine
Yet another rust audio library built with miniaudio and signalsmitch-stretch.

## Features
* Easy to use without rust trait-bs thingy.
* 3D supported with miniaudio's spatialization.
* Timestretch and Pitchshifting support on device, mixer and channel level with signalsmitch-stretch.
* Support DSP callbacks on device, channel and mixer level.

## Formats
Supported out of the box:
* mp3
* wav
* ogg (vorbis)
* ogg (opus) (unstable)

## Example
```rs
use estaudio::prelude*;

fn main() {
    let mut audio_device = AudioEngine::make_device()
        .build()
        .expect("Audio Device failed to created");

    let channel = AudioEngine::make_channel(Some(&audio_device))
        .file("./test.ogg")
        .build()
        .expect("Failed to create channel");

    channel.play().expect("Failed to play channel");

    while channel.is_playing() {
        // Sleep thread here
    }
}
```

## License
MIT or Apache 2.0

Excepts on each file in the `assets` folder has its own license.
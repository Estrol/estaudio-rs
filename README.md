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
The examples were available at folder `Examples`, you can try run it with `cargo run --example NAME`.

## License
MIT or Apache 2.0

Excepts on each file in the `assets` folder has its own license.
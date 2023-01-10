# Synja

Analog subtractive synth (VST3 Plugin) made with Rust.
More an experiment in making a VST and learning Rust than a production quality Synth, but it's getting there. Only tested on Windows (x64).

<video src='https://user-images.githubusercontent.com/2737503/211158871-d5475269-8ce2-4b32-a1af-c915af0e7a63.mov' width='700' ></video>

# Features

Typical mid-80's polyphonic synthesizer

- Two Oscillators + One LFO
- Unison
- AMP/Filter envelopes
- Mono/Poly mode with Portamento

# Building

Can be built via a regular `cargo build --release`, but using the xtask bundler is convenient and offers easier cross compilation. 
Install the bundler:

```
cargo install --git https://github.com/robbert-vdh/nih-plug.git cargo-nih-plug
```

Then to build the plugin

``` 
cargo nih-plug bundle synja --release
```

# Credits

The great VST3/CLAP plugin interface crate [nih-plug](https://github.com/RustAudio/vst-rs)

The fantastic [egui](https://github.com/emilk/egui)

LCD Display and knobs from [xTibor](https://github.com/xTibor/egui_extras_xt)

Moog style filter adapted from Antti Huovilainen's version via https://github.com/ddiakopoulos/MoogLadders

Kyle Dixon & Micahel Stein for the demo video tune
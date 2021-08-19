# TermDaw documentation

## Config

### General

The config is written as a `.toml` file.
It is read on startup and can't be refreshed.
If you change something you have to change termdaw again.
The config is project wide, you'll have a config for every project.
The config file is named `project.toml` and TermDaw will look for it in the directory it's been launched in.

### Example

```toml
[project]
name = "Example Project"

[settings]
main = "project.lua"
buffer_length = 1024
project_samplerate = 48000
normalize_on_refresh = false
```

### Fields

Field                   | Parent        | Optional      | Type  | Default Value
------------------------|---------------|---------------|-------|---------------
name                    | [project]     | Yes           | String| unnamed
main                    | [settings]    | No            | String|
buffer_length           | [settings]    | Yes           | Uint  | 1024
project_samplerate      | [settings]    | Yes           | Uint  | 44100
normalize_on_refresh    | [settings]    | Yes           | Bool  | True

## Tui

### General

TermDaw is controlled by a TUI (Terminal User Interface).
You can select commands with the arrow keys or you can type them (partially).

## Commands
- quit: close down TermDaw.
- refresh: reload the lua file and update internals to match it
- render: render audio and write it to disk
- normalize: perform normalization scan
- play: start playing audio from the current time on
- pause: stop playing audio but keep current time
- stop: stop playing audio and set time to 0
- skip: go 5 seconds forward in time
- prev: go 5 seconds backwards in time
- set: prompts you for a time (float) and will set the time to it if valid
- get: get the current time

## Lua

### General

The project itself is configured with

### Functions

#### Loading

- `set_length(seconds: float);` Sets the lenght of the render in seconds. You can listen past this point in the daw.
- `set_render_samplerate(sr: uint);` Sets the samplerate of the render. This is different than the samplerate of the project (internal, playback in daw, etc). You can set the project samplerate in the toml config.
- `set_output_file(file: string);` Sets the name of the rendered file.
- `load_sample(name: string, path: string);` Load a sample(.wav) found at the given path into the samplebank tagged with a name for further use.
- `load_midi_floww(name: string, path: string);` Load a midi file found at the given path into the flowwbank tagged with a name for further use.
- `load_lv2(name: string, uri: string);` Load a lv2 fx plugin found with the lv2 plugin URI and tag it with a name for further use. You can find all your lv2 plugins URI's with the cli program `lv2ls`.

#### Misc

- `parameter(plugin: string, name: string, value: float);` Set a parameter of a lv2 plugin where plugin is the name of the loaded plugin, name is the name of the parameter and the value what to assign it to.

#### Graph

##### Vertex

Every vertex has a:
- name: name of the vertex to reference it by
- gain: the volume of the vertex, can be over one
- angle: the angle of panning of the vertex. `0.0` is in the middle, `90.0` is full left and `-90.0` is full right.

##### Adsr Config
A adsr conf describes the amplitude of a sound over time (Attack, Decay, Sustain, Release).
The release part is used when the note is released according to the floww.
You contruct an adsr conf as an array of floats. You can either have 6 or 9 floats in it representing (attack_seconds, decay_seconds, decay_velocity, sustain_seconds, sustain_velocity, release_seconds) and  (standard_velocity, attack_seconds, attack_velocity, decay_seconds, decay_velocity, sustain_seconds, sustain_velocity, release_seconds, release_velocity) respectively. The seconds are how long those  parts last and the velocities are how loud the note is at that time. For a better understanding see [wikipedia](https://en.wikipedia.org/wiki/Envelope_(music)).

- `add_sum(name: string, gain: float, angle: float);` Add a summing vertex. It takes all inputs and sums them together.
- `add_normalize(name: string, gain: float, angle: float);` Add a normalize vertex. It takes all inputs and sums them, then normalizes the signal to be inbetween zero and one. So find out the mulitplier it has to use, you need to run the `normalize` command. You can let it do the normalize scan every refresh or not (see toml config). If somehow after some changes the audio distorts at the peaks, you need to can again.
- `add_sampleloop(name: string, gain: float, angle: float, sample: string);` take the sample by name and just loop it.
- `add_sample_multi(name: string, gain: float, angle: float, sample: string, floww: string, note: int);` Add a vertex that plays a sample when a note hits in a floww.
  - You can configure a specific midi note value that it will trigger on with the note argument. If you set it to -1 it will trigger on any note.
  - This vertex can play samples in parallel, if a note hits and the old one was not yet done both will play.
- `add_sample_lerp(name: string, gain: float, angle: float, sample: string, floww: string, note: int, lerp_length: int);` Add a vertex that plays a sample when a note hits in a floww.
  - You can configure a specific midi note value with the note argument to react to. If you set it to -1 it will trigger on any note.
  - If a new hit starts when the old one is still playing, the transision length is defined by lerp_length (length in sample frames).
- `add_debug_sine(name: string, gain: float, angle: float, floww: string);` A super simple synth that plays a pure sine wave. Just for testing and debugging. Doesn't even have attack and decay so it will destort every begin and end of note.
- `add_synth(name: string, gain: float, angle: float, floww: string, square_gain: float, square_z: float, square_adsr_conf: {float}, topflat_gain: float, topflat_z: float, topflat_adsr_conf: {float}, triangle_gain: float, triangle_adsr_conf: {float});` A synth vertex that emits sound given the floww.
  - square_gain: gain of the square wave oscilator
  - square_z: param going from zero to one, zero meaning completely square wave and one meaning completely sine wave. You can have values inbetween.
  - square_adsr_conf: an adsr config for the square oscilator
  - topflat_gain: gain of the top flat sine wave oscilator
  - topflat_z: param going from zero to one, zero meaning completely top flat sine wave and one meaning completely sine wave. You can have values inbetween.
  - topflat_adsr_conf: an adsr config for the square oscilator
  - triangle_gain: gain of the triangle wave oscilator
  - triangle_adsr_conf: adsr conf for the triangle oscilator
  - For a better understanding of the first two oscilators see [graphtoy](https://graphtoy.com/?f1(x,t)=min(sin(x),0)*2+1&v1=false&f2(x,t)=max(sin(x),0)*2-1&v2=false&f3(x,t)=0.4&v3=true&f4(x,t)=(min(sin(x),f3(0))+((1-f3(0))/2))*(2/(1+f3(0)))&v4=false&f5(x,t)=(max(sin(x),-f3(0))-((1-f3(0))/2))*(2/(1+f3(0)))&v5=false&f6(x,t)=clamp(sin(x),%20-f3(0),%20f3(0))%20*%20(1%20/%20f3(0))&v6=true&grid=true&coords=0,0,4.205926793776712)
- `add_lv2fx(name: string, gain: float, angle: float, wetness: float, plugin: string);` Adds a vertex that sums incomming audio and applies a lv2 fx plugin on it. wetness is how much of the new signal is mixed in. With 0.0 the vertex has no effect and with 1.0 the vertex outputs the signal after the plugin is aplied. With 0.5, for example, it is half the original signal and half the processed signal.
- `add_adsr(name: string, gain: float, angle: float, wetness: float, floww: string, use_off: bool, note: int, adsr_conf: {float});` This vertex sums the intputs and takes a floww. Based on the flow it applies the adsr envelope on the input signal and outputs it.
  - wettness: how much of the processed signal is mixed in 0.0 for none and 1.0 for full
  - use_off: whether to listen to the note off event off the floww
  - note: specific midi note value to trigger on, -1 for all notes
  - adsr_conf: the adsr config to apply on the audio
- `connect(a: string, b: string);` Takes two names of vertices and connects them to eachother. The output of a will be the intput for b.
- 'set_output(out: string);' Takes a name of a vertex and sets this to be the last vertex: the output of this vertex will be the final result.

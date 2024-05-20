# termdaw

A daw, in a terminal.
Termdaw is a (or should become a) terminal, graph based programmable pipeline
digital audio workstation that is friendly to automatization and algorithmic composition.

## MVP Goals

- Audio
  - [x] Render audio
    - Any samplerate: will up or down sample from project rate
    - 8, 16, 24 or 32 bit bitdepth
  - [x] Play back audio
- Terminal UX Workflow
  - [x] Controls: play, pause, stop
  - [x] Controls: set/get time, dash in time
  - [x] Controls: refresh, render
  - [x] View: Terminal logging, warnings, errors, colors
  - [x] Toml configuration
- Streaming Workflow
  - [x] Streaming mode (input through stdin)
- Structure
  - [x] Sample Bank
  - [x] Floww Bank
  - [x] Graph rendering structure
- Base Vertex, with:
  - [x] Gain
  - [x] Panning
  - [x] Input summation
- Vertex types
  - [x] Sum vertex
  - [x] Normalize vertex
  - [x] Sample loop vertex
  - [x] Envelope vertex
  - [x] Sample multi vertex
  - [x] Sample lerp vertex
  - [x] Lv2 fx vertex
  - [ ] Lv2 midi instrument vertex
  - [x] Debug Sine synth vertex
  - [x] Simple synth vertex
  - [x] Wavetable synth vertex
  - [x] Band pass/cut vertex
- Floww
  - [x] Read from midi
  - [ ] Read floww from floww description
  - [x] Read binary floww from stdin stream
  - [ ] Read human readable floww from stdin stream
- Lua
  - [x] To configure the project (sample rate, etc)
  - [x] Load resources (samples and plugins)
  - [x] Construct graph
  - [x] Refreshable: remove old, add new, keep same
- Docs
  - [x] Config documentation
  - [x] Examples

## Goals for later

- [ ] Multitype graph
  - [ ] In/Out ports
  - [ ] Type checker
  - [ ] Stereo type
  - [ ] Mono type
  - [ ] Floww type
  - [ ] Value type
- [ ] Value automation
- [ ] Lufs mastering tool
- [ ] Linear interpolation of floww notes
- [ ] Better scrolling through time handling of on/off notes
- [ ] Split vertex
- [ ] Active toggle on vertices
- [ ] Disable completely dry vertices
- [ ] Prune disabled vertices from the graph
- [ ] Multithreading

## Failed

- [ ] Bounded normalization: lv2 plugin's output can have more gain than input, no way to know how much.

## Example

```toml
[project]
name = "Example Project"

[settings]
buffer_length = 1024
project_samplerate = 48000
main = "project.lua"
```

```lua
-- Our lua file, where we do all the work
set_length(3.0);
set_render_samplerate(44100);
set_render_bitdepth(16);
set_output_file("outp.wav");

load_sample("snare", "/home/cody/doc/samples/drumnbass/snare-1/snare-1-v-9.wav");
load_sample("kick", "/home/cody/doc/samples/drumnbass/kick/kick-v-9.wav");

add_sampleloop("one", 1.0, 0.0, "snare");
add_sampleloop("two", 1.0, 0.0, "kick");
add_normalize("sum", 1.0, 0.0);

connect("one", "sum");
connect("two", "sum");

set_output("sum");
```

## License

```
Copyright (C) 2024 Cody Bloemhard

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <https://www.gnu.org/licenses/>.
```

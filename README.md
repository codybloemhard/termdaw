# termdaw
A daw, in a terminal.
Termdaw is a (or should become a) terminal, graph based programmable pipeline digital audio workstation that is friendly to automatization and AI/Algorithmic composition.

## MVP Goals
- Audio
  - [x] Render audio
    - Any samplerate: will up or down sample from project rate
    - 8, 16, 24 or 32 bit bitdepth
  - [ ] Play back audio
- Terminal UX
    - [ ] Controls: play, pause, stop
    - [ ] Controls: set time, dash in time
    - [ ] Controls: refresh, render
    - [ ] View: Terminal logging, warnings, errors, colors
- [x] Sample Bank
- [x] Graph rendering structure
- [ ] Midi Bank
- Base Vertex, with:
  - [x] Gain
  - [x] Panning
  - [x] Input summation
- Vertex types
  - [x] Sum vertex
  - [x] Normalize vertex
  - [x] Sample loop vertex
  - [ ] Sample midi vertex (emit sample on note)
  - [ ] Lv2 fx vertex
  - [ ] Lv2 midi instrument vertex
- Lua
  - [x] To configure the project(sample rate, etc)
  - [x] Load resources (samples and plugins)
  - [x] Construct graph
  - [ ] Refreshable: remove old, add new, keep same

## Goals for later
- [ ] Bound normalization
- [ ] Automation
- [ ] Midi pipeline
- [ ] Mono support

# termdaw
A daw, in a terminal.
Termdaw is a (or should become a) terminal, graph based programmable pipeline digital audio workstation that is friendly to automatization and AI/Algorithmic composition.

## MVP Goals
[x] Render audio
[ ] Play back audio
[ ] Terminal playback controls (play, pause, stop, set time, dash in time)
[x] Graph rendering structure
[x] Vertices, each with gain:
  [x] Sum vertex
  [x] Normalize vertex
  [ ] Bound normalization
  [ ] Panning vertex
  [x] Sample loop vertex
  [ ] Sample midi vertex (emit sample on note)
  [ ] Lv2 fx vertex
  [ ] Lv2 midi instrument vertex
[ ] Lua
  [ ] To configure the project(sample rate, etc)
  [ ] Load resources (samples and plugins)
  [ ] Construct graph

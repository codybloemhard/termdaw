# termdaw
A daw, in a terminal.
Termdaw is a (or should become a) terminal, graph based programmable pipeline digital audio workstation that is friendly to automatization and AI/Algorithmic composition.

## MVP Goals
- Terminal playback controls (play, pause, stop, set time, dash in time)
- Graph rendering structure
- Vertices, each with gain and panning:
  * Output vertex
  * Sample midi vertex (emit sample on note)
  * Lv2 midi instrument vertex
  * Sum vertex
  * Normalize vertex
  * Lv2 fx vertex
- Lua
  * To configure the project(sample rate, etc)
  * Load resources (samples and plugins)
  * Construct graph
- Play back audio and render audio

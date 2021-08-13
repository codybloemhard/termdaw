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

The project 

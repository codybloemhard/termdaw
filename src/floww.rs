use std::collections::{ HashMap };

use apres::{ MIDI, ApresError };
use apres::MIDIEvent::{ NoteOn, NoteOff };

pub fn midi_from_file(file: &str) -> Result<MIDI, ApresError>{
    MIDI::from_path(file)
}

pub type DrumFloww = Vec<(f32,f32,f32)>;

pub fn mono_midi_to_drum_floww(midi: MIDI) -> DrumFloww{
    let mut floww = Vec::new();
    let mut map = HashMap::new();
    for track in midi.get_tracks(){
        for (tick, id) in track{
            match midi.get_event(id){
                Some(NoteOn(_ch, note, vel)) => {
                    map.insert(note, (vel, tick));
                },
                Some(NoteOff(_ch, note, _vel)) => {
                    if let Some((on_vel, on_tick)) = map.get(&note){
                        floww.push((tick as f32, *on_vel as f32, (tick - on_tick) as f32));
                    }
                },
                _ => {  },
            }
        }
    }
    floww
}

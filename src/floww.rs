use std::collections::{ HashMap };

use apres::{ MIDI, ApresError };
use apres::MIDIEvent::{ NoteOn, NoteOff };

#[derive(Default)]
pub struct FlowwBank{
    sr: usize,
    drum_flowws: Vec<DrumFloww>,
    drum_start_indices: Vec<usize>,
    drum_names: HashMap<String, usize>,
}

impl FlowwBank{
    pub fn new(sr: usize) -> Self{
        Self{ sr: sr, ..Default::default() }
    }

    pub fn add_drum_floww(&mut self, name: String, path: &str){
        if let Ok(midi) = MIDI::from_path(path){
            let floww = mono_midi_to_drum_floww(midi, self.sr);
            self.drum_flowws.push(floww);
            self.drum_start_indices.push(0);
            self.drum_names.insert(name, self.drum_flowws.len() - 1);
        } else {
            println!("Could not read midi file: {}", path);
        }
    }

    pub fn get_index(&self, name: &str) -> Option<usize>{
        if let Some(index) = self.drum_names.get(name){
            Some(*index)
        } else {
            None
        }
    }

    pub fn set_time(&mut self, t: f32){
        let t_frame = (t * self.sr as f32).round() as usize;
        for (i, floww) in self.drum_flowws.iter().enumerate(){
            for (j, (frame, _, _)) in floww.iter().enumerate(){
                if frame > &t_frame{
                    self.drum_start_indices[i] = j;
                    break;
                }
            }
        }
    }
}

// (frame, note, vel)
pub type DrumFloww = Vec<(usize,f32,f32)>;

pub fn mono_midi_to_drum_floww(midi: MIDI, sr: usize) -> DrumFloww{
    let mut floww = Vec::new();
    for track in midi.get_tracks(){
        for (tick, id) in track{
            if let Some(NoteOn(_ch, note, vel)) = midi.get_event(id) {
                floww.push(((tick as f32 * sr as f32) as usize, note as f32, vel as f32));
            }
        }
    }
    floww
}

// pub fn mono_midi_to_drum_floww(midi: MIDI) -> DrumFloww{
//     let mut floww = Vec::new();
//     let mut map = HashMap::new();
//     for track in midi.get_tracks(){
//         for (tick, id) in track{
//             match midi.get_event(id){
//                 Some(NoteOn(_ch, note, vel)) => {
//                     map.insert(note, (vel, tick));
//                 },
//                 Some(NoteOff(_ch, note, _vel)) => {
//                     if let Some((on_vel, on_tick)) = map.get(&note){
//                         floww.push((tick as f32, *on_vel as f32, (tick - on_tick) as f32));
//                     }
//                 },
//                 _ => {  },
//             }
//         }
//     }
//     floww
// }

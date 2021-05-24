use std::collections::{ HashMap };

use apres::{ MIDI, ApresError };
use apres::MIDIEvent::{ NoteOn, NoteOff };

#[derive(Default)]
pub struct FlowwBank{
    sr: usize,
    bl: usize,
    frame: usize,
    block_index: usize,
    drum_flowws: Vec<DrumFloww>,
    drum_start_indices: Vec<usize>,
    drum_names: HashMap<String, usize>,
}

impl FlowwBank{
    pub fn new(sr: usize, bl: usize) -> Self{
        Self{ sr, bl, ..Default::default() }
    }

    pub fn reset(&mut self){
        self.frame = 0;
        self.block_index = 0;
        self.drum_flowws.clear();
        self.drum_start_indices.clear();
        self.drum_names.clear();
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

    fn set_start_indices_to_frame(&mut self, t_frame: usize, do_skip: bool){
        for (i, floww) in self.drum_flowws.iter().enumerate(){
            let skip = if do_skip{ self.drum_start_indices[i] }
            else { 0 };
            for (j, (frame, _, _)) in floww.iter().enumerate().skip(skip){
                if frame > &t_frame{
                    self.drum_start_indices[i] = j;
                    break;
                }
            }
        }
    }

    pub fn set_time(&mut self, t: f32){
        let t_frame = (t * self.sr as f32).round() as usize;
        self.set_start_indices_to_frame(t_frame, false);
        self.frame = t_frame;
    }

    pub fn set_time_to_next_block(&mut self){
        let frame = self.frame + self.bl;
        self.set_start_indices_to_frame(frame, true);
    }

    pub fn start_block(&mut self, index: usize){
        if index >= self.drum_flowws.len() { return; }
        self.block_index = self.drum_start_indices[index];
    }

    pub fn get_block(&mut self, index: usize, offset_frame: usize) -> Option<DrumPoint>{
        if index >= self.drum_flowws.len() { return None; }
        let next_event = self.drum_flowws[index][self.block_index];
        if next_event.0 == self.frame + offset_frame{
            if self.block_index + 1 < self.drum_flowws[index].len() {
                self.block_index += 1;
            }
            Some(next_event)
        } else {
            None
        }
    }
}

// (frame, note, vel)
pub type DrumPoint = (usize, f32, f32);
pub type DrumFloww = Vec<DrumPoint>;

pub fn mono_midi_to_drum_floww(midi: MIDI, sr: usize) -> DrumFloww{
    let mut floww = Vec::new();
    for track in midi.get_tracks(){
        for (tick, id) in track{
            if let Some(NoteOn(note, _what_is_this, vel)) = midi.get_event(id) {
                floww.push(((tick as f32 * sr as f32) as usize, note as f32, vel as f32));
            }
            if let Some(NoteOff(_, _, _)) = midi.get_event(id){
                print!("{}, ", tick);
            }
        }
    }
    floww.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
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

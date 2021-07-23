use apres::{ MIDI };
use apres::MIDIEvent::{ NoteOn, NoteOff, SetTempo };

use std::collections::{ HashMap };

#[derive(Default)]
pub struct FlowwBank{
    sr: usize,
    bl: usize,
    frame: usize,
    block_index: usize,
    flowws: Vec<Floww>,
    start_indices: Vec<usize>,
    names: HashMap<String, usize>,
}

impl FlowwBank{
    pub fn new(sr: usize, bl: usize) -> Self{
        Self{ sr, bl, ..Default::default() }
    }

    pub fn reset(&mut self){
        self.frame = 0;
        self.block_index = 0;
        self.flowws.clear();
        self.start_indices.clear();
        self.names.clear();
    }

    pub fn add_floww(&mut self, name: String, path: &str){
        if let Ok(midi) = MIDI::from_path(path){
            let floww = mono_midi_to_floww(midi, self.sr);
            self.flowws.push(floww);
            self.start_indices.push(0);
            self.names.insert(name, self.flowws.len() - 1);
        } else {
            println!("Could not read midi file: {}", path);
        }
    }

    pub fn get_index(&self, name: &str) -> Option<usize>{
        if let Some(index) = self.names.get(name){
            Some(*index)
        } else {
            None
        }
    }

    fn set_start_indices_to_frame(&mut self, t_frame: usize, do_skip: bool){
        for (i, floww) in self.flowws.iter().enumerate(){
            let skip = if do_skip{ self.start_indices[i] }
            else { 0 };
            for (j, (frame, _, _, _)) in floww.iter().enumerate().skip(skip){
                if frame >= &t_frame{
                    self.start_indices[i] = j;
                    break;
                }
            }
        }
    }

    pub fn set_time(&mut self, t: usize){
        self.set_start_indices_to_frame(t, false);
        self.frame = t;
    }

    pub fn set_time_to_next_block(&mut self){
        self.frame += self.bl;
        self.set_start_indices_to_frame(self.frame, true);
    }

    pub fn start_block(&mut self, index: usize){
        if index >= self.flowws.len() { return; }
        self.block_index = self.start_indices[index];
    }

    // returns Option<(note, vel)>
    pub fn get_block_drum(&mut self, index: usize, offset_frame: usize) -> Option<(f32, f32)>{
        if index >= self.flowws.len() { return None; }
        loop{
            if self.block_index >= self.flowws[index].len(){
                return None;
            }
            let next_event = self.flowws[index][self.block_index];
            // this skips events when multiple on values are in the same time frame
            if next_event.0 < self.frame + offset_frame{
                self.block_index += 1;
                continue;
            }
            if next_event.0 == self.frame + offset_frame{
                self.block_index += 1;
                // Only send through if it's a hit, ignore note off's
                if next_event.3 > 0.001{
                    return Some((next_event.2, next_event.3))
                }
            } else {
                return None;
            }
        }
    }

    // returns Vec<(on?, note, vel)>
    pub fn get_block_simple(&mut self, index: usize, offset_frame: usize) -> Vec<(bool, f32, f32)>{
        let mut res = Vec::new();
        if index >= self.flowws.len() { return res; }
        loop{
            if self.block_index >= self.flowws[index].len(){
                break;
            }
            let next_event = self.flowws[index][self.block_index];
            if next_event.0 == self.frame + offset_frame{
                self.block_index += 1;
                let on = next_event.3 > 0.001;
                res.push((on, next_event.2, next_event.3));
            } else {
                break;
            }
        }
        res
    }

    // returns Vec<(note, vel)>
    // pub fn get_block_continuous(&mut self, index: usize, offset_frame: usize) -> Vec<(f32, f32)>{
    // }
}

// (frame, id, note, vel)
pub type Point = (usize, usize, f32, f32);
pub type Floww = Vec<Point>;

pub fn mono_midi_to_floww(midi: MIDI, sr: usize) -> Floww{
    let ppqn = midi.get_ppqn() as f32;
    let mut time_mult = 1.0; // 60bpm per default
    let mut floww = Vec::new();
    for track in midi.get_tracks(){
        let mut time = 0.0;
        for (tick, id) in track{
            time += tick as f32 / ppqn * time_mult * sr as f32;
            let ev = midi.get_event(id);
            if let Some(NoteOn(_, note, vel)) = ev {
                floww.push((time as usize, note as usize, note as f32, vel as f32 / 127.0));
            }
            else if let Some(NoteOff(_, note, _)) = ev {
                floww.push((time as usize, note as usize, note as f32, 0.0));
            }
            else if let Some(SetTempo(t)) = ev {
                time_mult = t as f32 / 1_000_000.0;
            }
        }
    }
    floww
}


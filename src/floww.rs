use term_basics_linux::UC;

use std::collections::{ HashMap };

use ::floww::{ Floww, read_floww_from_midi, FlowwPacket, unpacket };

#[derive(Default)]
pub struct FlowwBank{
    sr: usize,
    bl: usize,
    frame: usize,
    block_index: usize,
    flowws: Vec<Floww>,
    start_indices: Vec<usize>,
    names: HashMap<String, usize>,
    stream_list: Vec<usize>,
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
        self.stream_list.clear();
    }

    fn declare_floww(&mut self, name: String, floww: Floww) -> usize{
        self.flowws.push(floww);
        self.start_indices.push(0);
        let index = self.flowws.len() - 1;
        self.names.insert(name, index);
        index
    }

    pub fn add_floww(&mut self, name: String, path: &str) -> Result<(), String>{
        if let Ok(floww) = read_floww_from_midi(path){
            self.declare_floww(name, floww);
            Ok(())
        } else {
            Err(format!("{r}Could not read midi file: {b}\"{x}\"{r}.",
                r = UC::Red, b = UC::Blue, x = path))
        }
    }

    pub fn declare_stream(&mut self, name: String){
        let index = self.declare_floww(name, vec![]);
        self.stream_list.push(index);
    }

    pub fn append_streams(&mut self, packets: Vec<FlowwPacket>) -> Vec<String>{
        unpacket(&mut self.flowws, &self.names, packets)
    }

    pub fn trim_streams(&mut self){
        for index in &self.stream_list{
            let start_index = self.start_indices[*index];
            self.flowws[*index].drain(..start_index);
        }
    }

    pub fn get_index(&self, name: &str) -> Option<usize>{
        self.names.get(name).copied()
    }

    fn set_start_indices_to_frame(&mut self, t_frame: usize, do_skip: bool){
        for (i, floww) in self.flowws.iter().enumerate(){
            let skip = if do_skip{ self.start_indices[i] }
            else { 0 };
            for (j, (_, t, _, _)) in floww.iter().enumerate().skip(skip){
                if ((t * self.sr as f32) as usize) >= t_frame{
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
            if ((next_event.1 * self.sr as f32) as usize) < self.frame + offset_frame{
                self.block_index += 1;
                continue;
            }
            if ((next_event.1 * self.sr as f32) as usize) == self.frame + offset_frame{
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
            if ((next_event.1 * self.sr as f32) as usize) == self.frame + offset_frame{
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


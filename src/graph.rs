use crate::sample::{ Sample, SampleBank };
use crate::floww::{ FlowwBank };
use crate::extensions::*;

use lv2hm::Lv2Host;

use std::collections::{ HashMap };

pub struct Graph{
    vertices: Vec<Vertex>,
    edges: Vec<Vec<usize>>,
    names: Vec<String>,
    name_map: HashMap<String, usize>,
    ran_status: Vec<bool>,
    output_vertex: Option<usize>,
    max_buffer_len: usize,
    sr: usize,
    t: usize,
}

impl Graph{
    pub fn new(max_buffer_len: usize, sr: usize) -> Self{
        Self{
            vertices: Vec::new(),
            edges: Vec::new(),
            name_map: HashMap::new(),
            names: Vec::new(),
            ran_status: Vec::new(),
            output_vertex: None,
            max_buffer_len,
            sr,
            t: 0,
        }
    }

    pub fn reset(&mut self){
        self.vertices.clear();
        self.edges.clear();
        self.name_map.clear();
        self.names.clear();
        self.ran_status.clear();
        self.output_vertex = None;
        self.t = 0;
    }

    pub fn add(&mut self, node: Vertex, name: String){
        self.vertices.push(node);
        self.ran_status.push(false);
        self.edges.push(Vec::new());
        let n = self.vertices.len() - 1;
        self.name_map.insert(name.clone(), n);
        self.names.push(name);
    }

    fn connect_internal(&mut self, a: usize, b: usize) -> bool{
        // basic checks
        if a == b { return false; }
        let len = self.vertices.len();
        if a >= len { return false; }
        if b >= len { return false; }
        if !self.vertices[b].has_input() { return false; }
        // loop detection:
        fn has_loop(x: usize, b: usize, edges: &[Vec<usize>]) -> bool{
            if x == b { return true; }
            for y in &edges[x]{
                if has_loop(*y, b, edges) { return true; };
            }
            false
        }
        if has_loop(a, b, &self.edges) { return false; }
        // connect a to b: a -> b, a into b
        // reverse: for such b we want to know which a's we should query
        self.edges[b].push(a);
        true
    }

    pub fn connect(&mut self, a: &str, b: &str) -> bool{
        let a_res = self.name_map.get(a);
        let b_res = self.name_map.get(b);
        if a_res.is_none() {
            println!("TermDaw: warning: vertex \"{}\" cannot be found and thus can't be connected.", a);
            return false;
        }
        if b_res.is_none() {
            println!("TermDaw: warning: vertex \"{}\" cannot be found and thus can't be connected to.", b);
            return false;
        }
        let a_index = *a_res.unwrap();
        let b_index = *b_res.unwrap();
        self.connect_internal(a_index, b_index)
    }

    fn run_vertex(&mut self, t: usize, sb: &SampleBank, fb: &mut FlowwBank, host: &mut Lv2Host, index: usize, is_scan: bool){
        if index >= self.vertices.len() { return; }
        if self.ran_status[index] { return; }
        self.ran_status[index] = true;
        let edges = self.edges[index].clone();
        for incoming in &edges{
            self.run_vertex(t, sb, fb, host, *incoming, is_scan);
        }
        // Vertex buffers exist as long at the graph exists: we never delete vertices
        // Safe: we mutate vertex A (&mut A) and read dat from incoming vertices [B] (&[B])
        // TODO: maybe use arena? https://crates.io/crates/typed-arena
        unsafe {
            let ins = edges.iter().map(|incoming|{
                &*(self.vertices[*incoming].read_buffer() as *const _)
            }).collect::<Vec<_>>();
            self.vertices[index].generate(t, self.sr, sb, fb, host, self.max_buffer_len, is_scan, ins);
        }
    }

    pub fn set_time(&mut self, time: usize){
        self.t = time;
        for v in &mut self.vertices{
            v.set_time(time);
        }
    }

    pub fn change_time(&mut self, delta: usize, plus: bool) -> usize{
        let new_time = if plus { self.t + delta }
        else { self.t - delta.min(self.t) };
        self.set_time(new_time);
        new_time
    }

    pub fn get_time(&self) -> usize{
        self.t
    }

    pub fn set_output(&mut self, vert: &str) -> bool{
        if let Some(index) = self.name_map.get(vert){
            self.output_vertex = Some(*index);
            true
        } else {
            false
        }
    }

    pub fn check_graph(&self) -> bool{
        let output = if let Some(out) = self.output_vertex{ out }
        else {
            println!("TermDaw: error: output vertex not found.");
            return false;
        };
        if self.edges[output].is_empty() && self.vertices[output].has_input(){
            println!("TermDaw: error: output receives no inputs.");
            return false;
        }
        let mut set = vec![false; self.vertices.len()];
        fn find_connected_component(x: usize, edges: &[Vec<usize>], set: &mut Vec<bool>){
            set[x] = true;
            for y in &edges[x]{
                find_connected_component(*y, edges, set);
            }
        }
        find_connected_component(output, &self.edges, &mut set);
        for (i, x) in set.into_iter().enumerate(){
            if x { continue; }
            println!("TermDaw: warning: vertex \"{}\" does not reach output.", self.names[i]);
        }
        true
    }

    fn reset_ran_stati(&mut self){
        for ran in &mut self.ran_status{
            *ran = false;
        }
    }

    pub fn render(&mut self, sb: &SampleBank, fb: &mut FlowwBank, host: &mut Lv2Host) -> Option<&Sample>{
        self.reset_ran_stati();
        if let Some(index) = self.output_vertex{
            self.run_vertex(self.t, sb, fb, host, index, false);
            self.t += self.max_buffer_len;
            Some(self.vertices[index].read_buffer())
        } else {
            None
        }
    }

    pub fn scan(&mut self, sb: &SampleBank, fb: &mut FlowwBank, host: &mut Lv2Host, chunks: usize){
        let i = if let Some(index) = self.output_vertex{ index }
        else { return; };
        fb.set_time(0);
        for j in 0..chunks {
            self.reset_ran_stati();
            self.run_vertex(j * self.max_buffer_len, sb, fb, host, i, true);
            fb.set_time_to_next_block();
        }
        self.set_time(0);
        fb.set_time(0);
    }
}

pub struct Vertex{
    buf: Sample,
    gain: f32,
    angle: f32,
    ext: VertexExt,
}

impl Vertex{
    pub fn new(bl: usize, gain: f32, angle: f32, ext: VertexExt) -> Self{
        Self{
            buf: Sample::new(bl),
            gain,
            angle: angle.min(90.0).max(-90.0),
            ext,
        }
    }

    fn read_buffer(&self) -> &Sample{
        &self.buf
    }

    fn generate(&mut self, t: usize, sr: usize, sb: &SampleBank, fb: &mut FlowwBank, host: &mut Lv2Host, len: usize, is_scan: bool, res: Vec<&Sample>){
        let len = self.buf.len().min(len);
        self.ext.generate(t, sr, sb, fb, host, self.gain, self.angle, &mut self.buf, len, res, is_scan);
    }

    // Whether or not you can connect another vertex to (into) this one
    fn has_input(&self) -> bool{
        self.ext.has_input()
    }

    pub fn set_time(&mut self, t: usize){
        self.ext.set_time(t);
    }
}


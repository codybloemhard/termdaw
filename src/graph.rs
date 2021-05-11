use crate::sample::{ Sample };

use std::collections::{ HashMap };

pub struct Graph<'a>{
    vertices: Vec<Vertex<'a>>,
    edges: Vec<Vec<usize>>,
    names: Vec<String>,
    name_map: HashMap<String, usize>,
    ran_status: Vec<bool>,
    max_buffer_len: usize,
    output_vertex: Option<usize>,
}

impl<'a> Graph<'a>{
    pub fn new(max_buffer_len: usize) -> Self{
        Self{
            vertices: Vec::new(),
            edges: Vec::new(),
            name_map: HashMap::new(),
            names: Vec::new(),
            ran_status: Vec::new(),
            max_buffer_len,
            output_vertex: None,
        }
    }

    pub fn add(&mut self, node: Vertex<'a>, name: String){
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

    fn run_vertex(&mut self, index: usize, is_scan: bool){
        if index >= self.vertices.len() { return; }
        if self.ran_status[index] { return; }
        self.ran_status[index] = true;
        let edges = self.edges[index].clone();
        for incoming in &edges{
            self.run_vertex(*incoming, is_scan);
        }
        // Vertex buffers exist as long at the graph exists: we never delete vertices
        // Safe: we mutate vertex A (&mut A) and read dat from incoming vertices [B] (&[B])
        // TODO: maybe use arena? https://crates.io/crates/typed-arena
        unsafe {
            let ins = edges.iter().map(|incoming|{
                &*(self.vertices[*incoming].read_buffer() as *const _)
            }).collect::<Vec<_>>();
            self.vertices[index].generate(self.max_buffer_len, is_scan, ins);
        }
    }

    pub fn set_time(&mut self, time: usize){
        for v in &mut self.vertices{
            v.set_time(time);
        }
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
        if self.edges[output].is_empty(){
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

    pub fn render(&mut self) -> Option<&Sample>{
        self.reset_ran_stati();
        if let Some(index) = self.output_vertex{
            self.run_vertex(index, false);
            Some(self.vertices[index].read_buffer())
        } else {
            None
        }
    }

    pub fn scan(&mut self, chunks: usize){
        let i = if let Some(index) = self.output_vertex{ index }
        else { return; };
        for _ in 0..chunks {
            self.reset_ran_stati();
            self.run_vertex(i, true);
        }
        self.set_time(0);
    }
}

pub struct Vertex<'a>{
    buf: Sample,
    gain: f32,
    angle: f32,
    ext: VertexExt<'a>,
}

impl<'a> Vertex<'a>{
    pub fn new(bl: usize, gain: f32, angle: f32, ext: VertexExt<'a>) -> Self{
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

    fn generate(&mut self, len: usize, is_scan: bool, res: Vec<&Sample>){
        let len = self.buf.len().min(len);
        self.ext.generate(self.gain, self.angle, &mut self.buf, len, res, is_scan);
    }

    // Whether or not you can connect another vertex to (into) this one
    fn has_input(&self) -> bool{
        self.ext.has_input()
    }

    pub fn set_time(&mut self, t: usize){
        self.ext.set_time(t);
    }

    pub fn set_gain(&mut self, gain: f32){
        self.gain = gain;
    }

    pub fn set_angle(&mut self, angle: f32){
        self.angle = angle.min(90.0).max(-90.0);
    }
}

pub enum VertexExt<'a>{
    Sum,
    Normalize{
        max: f32,
    },
    SampleLoop{
        sample: &'a Sample,
        playing: bool,
        t: usize,
    },
}

impl<'a> VertexExt<'a>{
    pub fn sum() -> Self{
        Self::Sum
    }

    pub fn normalize() -> Self{
        Self::Normalize{
            max: 0.0, // value on scan
        }
    }

    pub fn sample_loop(sample: &'a Sample) -> Self{
        Self::SampleLoop{
            sample,
            playing: true,
            t: 0,
        }
    }

    fn set_time(&mut self, time: usize){
        if let Self::SampleLoop { t, .. } = self{
            *t = time;
        }
    }

    fn generate(&mut self, gain: f32, angle: f32, buf: &mut Sample, len: usize, res: Vec<&Sample>, is_scan: bool){
        match self{
            Self::Sum => {
                sum_gen(buf, len, res);
            },
            Self::Normalize { max } => {
                normalize_gen(buf, len, res, max, is_scan);
            },
            Self::SampleLoop { playing, t, sample } => {
                sample_loop_gen(buf, len, playing, t, sample);
            },
        }
        buf.apply_angle(angle, len);
        buf.apply_gain(gain, len);
    }

    fn has_input(&self) -> bool{
        match self{
            Self::Sum => true,
            Self::Normalize { .. } => true,
            Self::SampleLoop { .. } => false,
        }
    }
}

fn sum_inputs(buf: &mut Sample, len: usize, res: Vec<&Sample>){
    buf.zero();
    for r in res{
        let l = r.len().min(len);
        for i in 0..l{
            buf.l[i] += r.l[i];
            buf.r[i] += r.r[i];
        }
    }
}

fn sum_gen(buf: &mut Sample, len: usize, res: Vec<&Sample>){
    sum_inputs(buf, len, res);
}

fn normalize_gen(buf: &mut Sample, len: usize, res: Vec<&Sample>, max: &mut f32, is_scan: bool){
    sum_inputs(buf, len, res);
    if is_scan{
        *max = buf.scan_max(len).max(*max);
    } else {
        buf.scale(len, 1.0 / *max);
    }
}

fn sample_loop_gen(buf: &mut Sample, len: usize, playing: &mut bool, t: &mut usize, sample: &Sample){
    if *playing{
        let l = sample.len();
        for i in 0..len{
            buf.l[i] = sample.l[(*t + i) % l];
            buf.r[i] = sample.r[(*t + i) % l];
        }
        *t += len;
    } else {
        buf.zero();
    }
}


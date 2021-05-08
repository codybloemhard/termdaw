use std::collections::{ HashMap };

use mlua::prelude::*;

mod sample;
use sample::*;

fn main() -> Result<(), String>{
    let bl = 1024;
    let mut sb = SampleBank::new(96000);
    sb.add("snare".to_owned(), "/home/cody/doc/samples/drumnbass/snare-1/snare-1-v-9.wav")?;
    sb.add("kick".to_owned(), "/home/cody/doc/samples/drumnbass/kick/kick-v-9.wav")?;
    let mut g = Graph::new(bl);

    g.add(Vertex::new(bl, 0.5, VertexExt::sample_loop(sb.get("snare").unwrap())), "one".to_owned());
    g.add(Vertex::new(bl, 0.5, VertexExt::sample_loop(sb.get("kick").unwrap())), "two".to_owned());
    g.add(Vertex::new(bl, 1.0, VertexExt::sum(true)), "sum".to_owned());
    g.connect("one", "sum");
    g.connect("two", "sum");
    g.set_output("sum");
    g.scan(800);

    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 96000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create("outp.wav", spec).unwrap();
    let amplitude = i16::MAX as f32;
    for _ in 0..800 {
        let chunk = g.render();
        if chunk.is_none() { continue; }
        let chunk = chunk.unwrap();
        for i in 0..1024{
            writer.write_sample((chunk.l[i] * amplitude) as i16).unwrap();
            writer.write_sample((chunk.r[i] * amplitude) as i16).unwrap();
        }
    }

    lua_test();
    Ok(())
}

pub struct Graph<'a>{
    vertices: Vec<Vertex<'a>>,
    edges: Vec<Vec<usize>>,
    names: HashMap<String, usize>,
    ran_status: Vec<bool>,
    max_buffer_len: usize,
    output_vertex: Option<usize>,
}

impl<'a> Graph<'a>{
    pub fn new(max_buffer_len: usize) -> Self{
        Self{
            vertices: Vec::new(),
            edges: Vec::new(),
            names: HashMap::new(),
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
        self.names.insert(name, n);
    }

    // TODO: check for cycles: if intoduce cycle, reject
    fn connect_internal(&mut self, a: usize, b: usize) -> bool{
        if a == b { return false; }
        let len = self.vertices.len();
        if a >= len { return false; }
        if b >= len { return false; }
        // connect a to b: a -> b, a into b
        // reverse: for such b we want to know which a's we should query
        self.edges[b].push(a);
        true
    }

    fn connect(&mut self, a: &str, b: &str) -> bool{
        let a_res = self.names.get(a);
        let b_res = self.names.get(b);
        if a_res.is_none() { return false; }
        if b_res.is_none() { return false; }
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
        if let Some(index) = self.names.get(vert){
            self.output_vertex = Some(*index);
            true
        } else {
            false
        }
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
    ext: VertexExt<'a>,
}

impl<'a> Vertex<'a>{
    fn new(bl: usize, gain: f32, ext: VertexExt<'a>) -> Self{
        Self{
            buf: Sample::new(bl),
            gain,
            ext,
        }
    }

    fn set_time(&mut self, t: usize){
        self.ext.set_time(t);
    }

    fn set_gain(&mut self, gain: f32){
        self.gain = gain;
    }

    fn read_buffer(&self) -> &Sample{
        &self.buf
    }

    fn generate(&mut self, len: usize, is_scan: bool, res: Vec<&Sample>){
        self.ext.generate(self.gain, &mut self.buf, len, res, is_scan);
    }
}

pub enum VertexExt<'a>{
    Sum{
        normalize: bool,
        max: f32,
    },
    SampleLoop{
        sample: &'a Sample,
        playing: bool,
        t: usize,
    },
}

impl<'a> VertexExt<'a>{
    fn sum(normalize: bool) -> Self{
        Self::Sum{
            normalize,
            max: 0.0, // value on scan
        }
    }

    fn sample_loop(sample: &'a Sample) -> Self{
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

    fn set_playing(&mut self, new_playing: bool){
        if let Self::SampleLoop { playing, .. } = self{
            *playing = new_playing;
        }
    }

    fn generate(&mut self, gain: f32, buf: &mut Sample, len: usize, res: Vec<&Sample>, is_scan: bool){
        match self{
            Self::Sum { normalize, max } => {
                sum_gen(gain, buf, len, res, *normalize, max, is_scan);
            },
            Self::SampleLoop { playing, t, sample } => {
                sample_loop_gen(gain, buf, len, playing, t, sample);
            },
        }
    }
}

fn sum_gen(gain: f32, buf: &mut Sample, len: usize, res: Vec<&Sample>, normalize: bool, max: &mut f32, is_scan: bool){
    buf.zero();
    let len = buf.len().min(len);
    for r in res{
        let l = r.len().min(len);
        for i in 0..l{
            buf.l[i] += r.l[i];
            buf.r[i] += r.r[i];
        }
    }
    if normalize{
        if is_scan{
            *max = buf.scan_max(len).max(*max);
        } else {
            buf.scale(len, 1.0 / *max);
        }
        println!("max: {}", max);
    }
    buf.apply_gain(gain, len);
}

fn sample_loop_gen(gain: f32, buf: &mut Sample, len: usize, playing: &mut bool, t: &mut usize, sample: &Sample){
    if *playing{
        let l = sample.len();
        let len = buf.len().min(len);
        for i in 0..len{
            buf.l[i] = sample.l[(*t + i) % l] * gain;
            buf.r[i] = sample.r[(*t + i) % l] * gain;
        }
        *t += len;
    } else {
        buf.zero();
    }
}

fn lua_test() -> LuaResult<()> {
    let lua = Lua::new();

    let map_table = lua.create_table()?;

    let greet = lua.create_function(|_, name: String| {
        println!("Hello, {}!", name);
        Ok(())
    });

    map_table.set(1, "one")?;
    map_table.set("two", 2)?;

    lua.globals().set("map_table", map_table)?;
    lua.globals().set("greet", greet.unwrap())?;

    lua.load("for k,v in pairs(map_table) do print(k,v) end").exec()?;
    lua.load("greet(\"haha yes\")").exec()?;

    println!("Hello, world!");
    Ok(())
}


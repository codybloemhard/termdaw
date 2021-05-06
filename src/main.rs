use std::collections::{ HashMap };

use rubato::{ Resampler, SincFixedIn, InterpolationType, InterpolationParameters, WindowFunction };
use mlua::prelude::*;

fn main() -> Result<(), String>{
    let mut sample_bank = SampleBank::new(96000);
    sample_bank.add("snare".to_owned(), "/home/cody/doc/samples/drumnbass/snare-1/snare-1-v-9.wav")?;
    lua_test();
    Ok(())
}

pub struct Graph{
    vertices: Vec<Box<dyn Vertex>>,
    edges: Vec<Vec<usize>>,
    names: HashMap<String, usize>,
    ran_status: Vec<bool>,
    max_buffer_len: usize,
}

impl Graph{
    pub fn new(max_buffer_len: usize) -> Self{
        Self{
            vertices: Vec::new(),
            edges: Vec::new(),
            names: HashMap::new(),
            ran_status: Vec::new(),
            max_buffer_len,
        }
    }

    pub fn add(&mut self, node: Box<dyn Vertex>, name: String){
        self.vertices.push(node);
        self.edges.push(Vec::new());
        let n = self.vertices.len() - 1;
        self.names.insert(name, n);
    }

    pub fn connect(&mut self, a: usize, b: usize){
        let len = self.vertices.len();
        if a >= len { return; }
        if b >= len { return; }
        self.edges[a].push(b);
    }

    pub fn run_vertex(&mut self, index: usize){
        if index >= self.vertices.len() { return; }
        if !self.ran_status[index] {
            self.ran_status[index] = true;
            let v = &mut self.vertices[index];
            v.generate(self.max_buffer_len, vec![]);
        }
    }
}

pub trait Vertex{
    fn read_buffer(&self) -> &Sample;
    fn generate(&mut self, len: usize, res: Vec<&Sample>);
}

pub struct SampleBank{
    sample_rate: usize,
    samples: Vec<Sample>,
    names: HashMap<String, usize>,
}

impl SampleBank{
    pub fn new(sample_rate: usize) -> Self{
        Self{
            sample_rate,
            samples: Vec::new(),
            names: HashMap::new(),
        }
    }

    pub fn add(&mut self, name: String, file: &str) -> Result<(), String>{
        if self.names.get(&name).is_some() {
            return Err(format!("TermDaw: SampleBank: there is already an sample with name \"{}\" present.", name));
        }
        let mut reader = if let Ok(reader) = hound::WavReader::open(file){
            reader
        } else {
            return Err(format!("TermDaw: SampleBank: could not open file {}.", file));
        };
        let specs = reader.spec();
        if specs.channels != 2 {
            return Err(format!("TermDaw: SampleBank: only stereo samples are supported yet, found {} channels.", specs.channels));
        }
        let sr = specs.sample_rate as usize;
        let bd = specs.bits_per_sample;
        let mut l = Vec::new();
        let mut r = Vec::new();
        let mut c = 0;
        if specs.sample_format == hound::SampleFormat::Float{
            for s in reader.samples::<f32>(){
                if s.is_err() { continue; }
                let s = s.unwrap();
                if c == 0 {
                    l.push(s);
                    c = 1;
                } else {
                    r.push(s);
                    c = 0;
                }
            }
        } else {
            let max = ((1 << bd) / 2 - 1) as f32;
            for s in reader.samples::<i32>(){
                if s.is_err() { continue; }
                let s = s.unwrap() as f32 / max;
                if c == 0 {
                    l.push(s);
                    c = 1;
                } else {
                    r.push(s);
                    c = 0;
                }
            }
        }
        if sr != self.sample_rate{ // need to resample
            // no idea what is means but comes from the example lol
            let params = InterpolationParameters {
                sinc_len: 256,
                f_cutoff: 0.95,
                interpolation: InterpolationType::Nearest,
                oversampling_factor: 160,
                window: WindowFunction::BlackmanHarris2,
            };
            let mut resampler = SincFixedIn::<f32>::new(
                self.sample_rate as f64 / sr as f64,
                params, l.len(), 2
            );
            let waves_in = vec![l, r];
            let mut waves_out = resampler.process(&waves_in).unwrap();
            l = std::mem::replace(&mut waves_out[0], Vec::new());
            r = std::mem::replace(&mut waves_out[1], Vec::new());
        }
        match Sample::from(l, r){
            Ok(sample) => {
                self.samples.push(sample);
                self.names.insert(name, self.samples.len() - 1);
                Ok(())
            },
            Err(e) => {
                Err(e)
            }
        }
    }

    pub fn get_sample(&self, name: &str) -> Option<&Sample>{
        if let Some(index) = self.names.get(name){
            Some(&self.samples[*index])
        } else {
            None
        }
    }
}

pub struct Sample{
    l: Vec<f32>,
    r: Vec<f32>,
}

impl Sample{
    pub fn new(bl: usize) -> Self{
        Self{
            l: Vec::with_capacity(bl),
            r: Vec::with_capacity(bl),
        }
    }

    pub fn from(l: Vec<f32>, r: Vec<f32>) -> Result<Self, String>{
        if l.len() != r.len() {
            return Err(format!("TermDaw: Sample::from: l and r do not have the same length: {} and {}.", l.len(), r.len()));
        }
        if l.is_empty(){
            return Err("TermDaw: Sample::from: l and r have length 0.".to_owned());
        }
        Ok(Self{ l, r })
    }

    pub fn len(&self) -> usize{
        self.l.len()
    }

    pub fn is_empty(&self) -> bool{
        self.l.is_empty()
    }

    pub fn clear(&mut self){
        self.l.clear();
        self.r.clear();
    }
}

pub struct SumVertex{
    buf: Sample,
}

impl SumVertex{
    pub fn new(bl: usize) -> Self{
        Self{
            buf: Sample::new(bl),
        }
    }
}

impl Vertex for SumVertex{
    fn read_buffer(&self) -> &Sample{
        &self.buf
    }

    fn generate(&mut self, len: usize, res: Vec<&Sample>){
        self.buf.clear();
        let len = self.buf.len().min(len);
        for r in res{
            let l = r.len().min(len);
            for i in 0..l{
                self.buf.l[i] += r.l[i];
                self.buf.r[i] += r.r[i];
            }
        }
    }
}

pub struct SampleLoopVertex<'a>{
    sample: &'a Sample,
    playing: bool,
    t: usize,
    buf: Sample,
}

impl<'a> SampleLoopVertex<'a>{
    pub fn new(sample: &'a Sample, bl: usize) -> Self{
        Self{
            sample,
            playing: false,
            t: 0,
            buf: Sample::new(bl),
        }
    }

    pub fn set_time(&mut self, t: usize){
        self.t = t;
    }

    pub fn set_playing(&mut self, playing: bool){
        self.playing = playing;
    }
}

impl Vertex for SampleLoopVertex<'_>{
    fn read_buffer(&self) -> &Sample{
        &self.buf
    }

    fn generate(&mut self, len: usize, _: Vec<&Sample>){
        if self.playing{
            let l = self.sample.len();
            for i in 0..self.buf.len().min(len){
                self.buf.l[i] = self.sample.l[(self.t + i) % l];
                self.buf.r[i] = self.sample.r[(self.t + i) % l];
            }
            self.t += l;
        } else {
            self.buf.clear();
        }
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


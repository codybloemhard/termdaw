use std::collections::{ HashMap };

use mlua::prelude::*;

fn main(){
    lua_test();
}

pub struct Graph{
    vertices: Vec<Box<dyn Node>>,
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

    pub fn add(&mut self, node: Box<dyn Node>, name: String){
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

pub type NodeRes<'a> = (usize, &'a[f32], &'a[f32]);

pub trait Node{
    fn read_buffer(&self) -> NodeRes;
    fn generate(&mut self, len: usize, res: Vec<NodeRes>);
}

pub struct Sample{
    l: Vec<f32>,
    r: Vec<f32>,
}

impl Sample{
    pub fn len(&self) -> usize{
        self.l.len()
    }

    pub fn is_empty(&self) -> bool{
        self.l.is_empty()
    }
}

pub struct SumNode{
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
}

impl SumNode{
    pub fn new(bl: usize) -> Self{
        Self{
            buf_l: Vec::with_capacity(bl),
            buf_r: Vec::with_capacity(bl),
        }
    }
}

impl Node for SumNode{
    fn read_buffer(&self) -> NodeRes{
        (self.buf_l.len(), &self.buf_l, &self.buf_r)
    }

    fn generate(&mut self, len: usize, res: Vec<NodeRes>){
        self.buf_l.clear();
        self.buf_r.clear();
        let len = self.buf_l.len().min(len);
        for (rlen, rl, rr) in res{
            let l = rlen.min(len);
            for i in 0..l{
                self.buf_l[i] += rl[i];
                self.buf_r[i] += rr[i];
            }
        }
    }
}

pub struct SampleLoopNode{
    sample: Sample,
    playing: bool,
    t: usize,
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
}

impl SampleLoopNode{
    pub fn new(sample: Sample, bl: usize) -> Self{
        Self{
            sample,
            playing: false,
            t: 0,
            buf_l: Vec::with_capacity(bl),
            buf_r: Vec::with_capacity(bl),
        }
    }

    pub fn set_time(&mut self, t: usize){
        self.t = t;
    }

    pub fn set_playing(&mut self, playing: bool){
        self.playing = playing;
    }
}

impl Node for SampleLoopNode{
    fn read_buffer(&self) -> NodeRes{
        (self.buf_l.len(), &self.buf_l, &self.buf_r)
    }

    fn generate(&mut self, len: usize, _: Vec<NodeRes>){
        if self.playing{
            let l = self.sample.len();
            for i in 0..self.buf_l.len().min(len){
                self.buf_l[i] = self.sample.l[(self.t + i) % l];
                self.buf_r[i] = self.sample.r[(self.t + i) % l];
            }
            self.t += l;
        } else {
            self.buf_l.clear();
            self.buf_r.clear();
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


// IceVec, a frozen size vector
// struct IceVec<T>(Vec<T>,usize);
//
// impl<T> IceVec<T>{
//     pub fn new(len: usize) -> Self{
//         Self(Vec::<T>::with_capacity(len),len)
//     }
//
//     pub fn len(&self) -> Self{
//         self.0.len()
//     }
//
//     pub fn push(&mut self, v: T) -> bool{
//         if self.len()
//     }
// }
//

use std::{
    collections::{ HashMap, HashSet },
    io::prelude::*,
    fs::File,
};

use zen_colour::*;

pub type Buffer = Vec<u8>;

pub struct BufferBank{
    buffers: Vec<Buffer>,
    names: HashMap<String, usize>,
    marked: HashSet<usize>,
}

impl BufferBank{
    pub fn new() -> Self{
        Self{
            buffers: Vec::new(),
            names: HashMap::new(),
            marked: HashSet::new(),
        }
    }

    pub fn add(&mut self, name: String, file_path: &str) -> Result<(), String>{
        if self.names.contains_key(&name) {
            return Err(format!(
                "{r}TermDaw: BufferBank: there is already a blob with name {b}\"{n}\"{r} present.",
                r = RED, b = BLUE, n = name
            ));
        }

        let mut buffer = Vec::new();
        let mut file = if let Ok(file) = File::open(file_path) { file }
        else {
            return Err(format!(
                "{r}TermDaw: BufferBank: could open read file {b}\"{f}\"{r}.",
                r = RED, b = BLUE, f = file_path
            ));
        };
        if file.read_to_end(&mut buffer).is_err() {
            return Err(format!(
                "{r}TermDaw: BufferBank: could not read file {b}\"{f}\"{r}.",
                r = RED, b = BLUE, f = file_path
            ));
        }

        self.buffers.push(buffer);
        self.names.insert(name, self.buffers.len() - 1);
        Ok(())
    }

    pub fn mark_dead(&mut self, name: &str){
        if let Some(index) = self.names.get(name){
            self.marked.insert(*index);
        }
    }

    pub fn refresh(&mut self){
        if self.marked.is_empty() { return; }
        let mut new_map = HashMap::new();
        let mut new_vec = Vec::new();
        let names = std::mem::take(&mut self.names);
        for (name, index) in names{
            if self.marked.contains(&index) { continue; }
            let buffer = std::mem::take(&mut self.buffers[index]);
            new_vec.push(buffer);
            new_map.insert(name, new_vec.len() - 1);
        }
        self.names = new_map;
        self.buffers = new_vec;
        self.marked.clear();
    }

    pub fn get_index(&self, name: &str) -> Option<usize>{
        self.names.get(name).copied()
    }

    pub fn get_buffer(&self, index: usize) -> &Buffer{
        &self.buffers[index]
    }
}


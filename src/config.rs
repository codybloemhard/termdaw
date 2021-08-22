use std::fs::File;
use std::io::Read;

use serde::Deserialize;

impl Config{
    pub fn read(path: &str) -> Self{
        let mut file = File::open(path).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        let config: Config = toml::from_str(&contents).unwrap();
        config
    }
}

#[derive(Deserialize, Clone)]
pub struct Config{
    pub project: Project,
    pub settings: Settings,
}

#[derive(Deserialize, Clone)]
pub struct Project{
    name: Option<String>,
}

impl Project{
    pub fn name(&self) -> String{
        self.name.clone().unwrap_or_else(|| String::from("unnamed"))
    }
}

#[derive(Deserialize, Clone)]
pub struct Settings{
    pub main: String,
    buffer_length: Option<usize>,
    project_samplerate: Option<usize>,
}

impl Settings{
    pub fn buffer_length(&self) -> usize{
        self.buffer_length.unwrap_or(1024)
    }

    pub fn project_samplerate(&self) -> usize{
        self.project_samplerate.unwrap_or(44100)
    }
}


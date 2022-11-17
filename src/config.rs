use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde::Deserialize;

impl Config{
    pub fn read(path: &Path) -> Self{
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

#[derive(Clone,Copy,PartialEq,Eq)]
pub enum WorkFlow{ Manual, Stream }


impl std::fmt::Display for WorkFlow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self{
            WorkFlow::Manual => "manual",
            WorkFlow::Stream => "stream",
        })
    }
}

#[derive(Deserialize, Clone)]
pub struct Settings{
    pub main: String,
    buffer_length: Option<usize>,
    project_samplerate: Option<usize>,
    workflow: Option<String>,
}

impl Settings{
    pub fn buffer_length(&self) -> usize{
        self.buffer_length.unwrap_or(1024)
    }

    pub fn project_samplerate(&self) -> usize{
        self.project_samplerate.unwrap_or(44100)
    }

    pub fn workflow(&self) -> WorkFlow{
        if let Some(string) = &self.workflow{
            match string.as_ref() {
                "stream" => WorkFlow::Stream,
                _ => WorkFlow::Manual,
            }
        } else {
            WorkFlow::Manual
        }
    }
}


use std::fs::File;
use std::io::{ Read };

use mlua::prelude::*;
use sdl2::audio::AudioSpecDesired;
use lv2hm::Lv2Host;
use term_basics_linux::*;

mod sample;
mod graph;
mod floww;
mod extensions;
mod adsr;
mod synth;
mod config;
mod state;
mod bufferbank;
mod ui_workflow;
mod stream_workflow;

use sample::*;
use graph::*;
use crate::floww::*;
use config::*;
use state::*;
use bufferbank::*;
use ui_workflow::*;
use stream_workflow::*;

fn main(){
    let config = Config::read("project.toml");

    println!("{}TermDaw: loading {}\"{}\"{} with \n\tbuffer_length = {}{}{} \n\tproject_samplerate = {}{}{} \n\tworkflow = {}{}{} \n\tmain = {}\"{}\"{}",
        UC::Std, UC::Blue, config.project.name(), UC::Std,
        UC::Blue, config.settings.buffer_length(), UC::Std,
        UC::Blue, config.settings.project_samplerate(), UC::Std,
        UC::Blue, config.settings.workflow(), UC::Std,
        UC::Blue, config.settings.main, UC::Std);

    let mut file = match File::open(&config.settings.main){
        Ok(f) => f,
        Err(e) => {
            println!("{}Error: could not open main lua file: {}\"{}\"{}.",
                UC::Red, UC::Blue, config.settings.main, UC::Red);
            println!("{}\t{}", UC::Red, e);
            return;
        }
    };
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    std::mem::drop(file);

    let proj_sr = config.settings.project_samplerate();
    let buffer_len = config.settings.buffer_length();
    let workflow = config.settings.workflow();

    let mut state = State{
        lua: Lua::new(),
        sb: SampleBank::new(proj_sr),
        g: Graph::new(config.settings.buffer_length(), proj_sr),
        host: Lv2Host::new(1000, buffer_len * 2, proj_sr), // acount for l/r
        fb: FlowwBank::new(proj_sr, buffer_len),
        bb: BufferBank::new(),
        contents,
        config,
        loaded: false,
        cs: 0,
        render_sr: 48000,
        bd: 16,
        output_vertex: String::new(),
        output_file: String::from("outp.wav"),
        cur_samples: Vec::new(),
        cur_resources: Vec::new(),
        cur_lv2plugins: Vec::new(),
        cur_lv2params: Vec::new(),
    };
    state.refresh();

    let sdl_context = match sdl2::init(){
        Ok(x) => x,
        Err(e) => {
            println!("{}Error: can't initialize sdl2 context.", UC::Red);
            println!("{}\t{}", UC::Red, e);
            return;
        }
    };
    let audio_subsystem = match sdl_context.audio(){
        Ok(x) => x,
        Err(e) => {
            println!("{}Error: can't get sdl audio subsystem.", UC::Red);
            println!("{}\t{}", UC::Red, e);
            return;
        }
    };
    let desired_spec = AudioSpecDesired {
        freq: Some(proj_sr as i32),
        channels: Some(2),
        samples: None,
    };
    let device = match audio_subsystem.open_queue::<f32, _>(None, &desired_spec){
        Ok(x) => x,
        Err(e) => {
            println!("{}Error: can't open sdl audio queue.", UC::Red);
            println!("{}\t{}", UC::Red, e);
            return;
        }
    };

    match workflow{
        WorkFlow::Manual => run_ui_workflow(proj_sr, buffer_len, state, device),
        WorkFlow::Stream => run_stream_workflow(proj_sr, buffer_len, state, device),
    }
}


use mlua::prelude::*;
use sdl2::audio::AudioSpecDesired;
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
mod lv2;

use {
    sample::*,
    graph::*,
    floww::*,
    config::*,
    state::*,
    bufferbank::*,
    ui_workflow::*,
    stream_workflow::*,
};

#[cfg(feature = "lv2")]
use lv2::Lv2Host;

use std::{
    fs::File,
    io::Read,
    path::Path,
};

fn main(){
    let args: Vec<String> = std::env::args().collect();
    let wdir = if args.len() > 1{
        args[1].to_owned()
    } else {
        "./".to_owned()
    };
    let wpath = Path::new(&wdir);
    let config = Config::read(&wpath.join("project.toml"));

    println!("{s}TermDaw: loading {b}\"{x}\"{s} with \n\tbuffer_length = {b}{y}{s} \n\tproject_samplerate = {b}{z}{s} \n\tworkflow = {b}{w}{s} \n\tworkdir = {b}{v}{s} \n\tmain = {b}\"{u}\"{s}",
        s = UC::Std, b = UC::Blue,
        x = config.project.name(),
        y = config.settings.buffer_length(),
        z = config.settings.project_samplerate(),
        w = config.settings.workflow(),
        v = wdir,
        u = config.settings.main);

    let main_path = wpath.join(&config.settings.main);
    let mut file = match File::open(main_path.clone()){
        Ok(f) => f,
        Err(e) => {
            println!("{r}Error: could not open main lua file: {b}\"{x:#?}\"{r}.",
                r = UC::Red, b = UC::Blue, x = main_path);
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
        #[cfg(feature = "lv2")]
        host: Lv2Host::new(1000, buffer_len * 2, proj_sr), // acount for l/r
        #[cfg(not(feature = "lv2"))]
        host: (),
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
        wdir,
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
    let desired_spec = AudioSpecDesired{
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


use std::fs::File;
use std::io::{ Read, Cursor };
use std::thread;
use std::sync::mpsc;
use std::time::{ Duration, Instant };

use mlua::prelude::*;
use skim::prelude::*;
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

use sample::*;
use graph::*;
use floww::*;
use config::*;
use state::*;
use bufferbank::*;

fn main(){
    let config = Config::read("project.toml");

    println!("{}TermDaw: loading {}\"{}\"{} with \n\tbuffer_length = {}{}{} \n\tproject_samplerate = {}{}{} \n\tmain = {}\"{}\"{}",
        UC::Std, UC::Blue, config.project.name(), UC::Std,
        UC::Blue, config.settings.buffer_length(), UC::Std,
        UC::Blue, config.settings.project_samplerate(), UC::Std,
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
    let mut playing = false;
    let mut since = Instant::now();
    let mut millis_generated = 0f32;

    let (transmit_to_ui, receive_in_ui) = mpsc::channel();
    let (transmit_to_main, receive_in_main) = mpsc::channel();

    thread::spawn(move || {
        let options = SkimOptionsBuilder::default()
            .height(Some("8%")).build().unwrap();
        let input = "quit\nrender\nrefresh\nnormalize\nplay\npause\nstop\n>skip\n<prev\nset\nget\nnorm-vals".to_string();
        let item_reader = SkimItemReader::default();
        loop{
            let items = item_reader.of_bufread(Cursor::new(input.clone()));
            let selected_items = Skim::run_with(&options, Some(items))
                .map(|out| out.selected_items)
                .unwrap_or_else(Vec::new);

            if let Some(item) = selected_items.get(0){
                let command = item.output();
                println!("{}---- {}", UC::Magenta, command);
                let tmsg = if command == "quit" { ThreadMsg::Quit }
                else if command == "refresh" { ThreadMsg::Refresh }
                else if command == "render" { ThreadMsg::Render }
                else if command == "normalize" { ThreadMsg::Normalize }
                else if command == "play" { ThreadMsg::Play }
                else if command == "pause" { ThreadMsg::Pause }
                else if command == "stop" { ThreadMsg::Stop }
                else if command == ">skip" { ThreadMsg::Skip }
                else if command == "<prev" { ThreadMsg::Prev }
                else if command == "norm-vals" { ThreadMsg::NormVals }
                else if command == "set" {
                    let raw = input_field();
                    let time: Option<f32> = string_to_value(&raw);
                    if let Some(float) = time{
                        if float >= 0.0{
                            let t = (float * proj_sr as f32) as usize;
                            ThreadMsg::Set(t)
                        } else {
                            println!("{}Error: time needs to be positive.", UC::Red);
                            ThreadMsg::None
                        }
                    } else {
                        println!("{}Error: could not parse time, did not set time.", UC::Red);
                        ThreadMsg::None
                    }
                }
                else if command == "get" { ThreadMsg::Get }
                else { ThreadMsg::None };
                transmit_to_main.send(tmsg).unwrap();
            } else {
                println!("{}TermDaw: command not found!", UC::Red);
                continue;
            }
            for received in &receive_in_ui{
                if received == ThreadMsg::Ready{
                    break;
                }
            }
        }
    });

    loop {
        if let Ok(rec) = receive_in_main.try_recv(){
            macro_rules! check_loaded{
                ($b:block) => {
                    if !state.loaded{
                        println!("{}State not loaded!", UC::Red);
                    } else {
                        $b;
                    }
                }
            }
            match rec{
                ThreadMsg::Quit => {
                    break;
                },
                ThreadMsg::Refresh => {
                    state.refresh();
                    playing = false;
                    device.clear();
                    device.pause();
                },
                ThreadMsg::Render => {
                    check_loaded!({
                        device.clear();
                        device.pause();
                        playing = false;
                        state.render();
                    });
                },
                ThreadMsg::Normalize => {
                    check_loaded!({
                        device.clear();
                        device.pause();
                        playing = false;
                        state.scan_exact();
                    });
                },
                ThreadMsg::Play => {
                    check_loaded!({
                        playing = true;
                        since = Instant::now();
                        millis_generated = 0.0;
                        device.resume();
                    });
                },
                ThreadMsg::Pause => {
                    playing = false;
                    device.pause();
                },
                ThreadMsg::Stop => {
                    check_loaded!({
                        playing = false;
                        device.pause();
                        device.clear();
                        state.g.set_time(0);
                        state.fb.set_time(0);
                    });
                },
                ThreadMsg::Skip => {
                    check_loaded!({
                        device.clear();
                        let time = state.g.change_time(5 * proj_sr, true);
                        state.fb.set_time(time);
                    });
                }
                ThreadMsg::Prev => {
                    check_loaded!({
                        device.clear();
                        let time = state.g.change_time(5 * proj_sr, false);
                        state.fb.set_time(time);
                    });
                }
                ThreadMsg::Set(time) => {
                    check_loaded!({
                        device.clear();
                        state.g.set_time(time);
                        state.fb.set_time(time);
                    });
                }
                ThreadMsg::Get => {
                    check_loaded!({
                        let t = state.g.get_time();
                        let tf = t as f32 / proj_sr as f32;
                        println!("{}Frame: {}{}{}, Time: {}{}",
                            UC::Std, UC::Blue, t, UC::Std, UC::Blue, tf);
                    });
                }
                ThreadMsg::NormVals => {
                    check_loaded!({
                        state.g.print_normalization_values();
                    });
                }
                _ => {}
            }
            transmit_to_ui.send(ThreadMsg::Ready).unwrap();
        }
        if playing{
            if !state.loaded {
                playing = false;
            } else {
                let time_since = since.elapsed().as_millis() as f32;
                // render half second in advance to be played
                while time_since > millis_generated - 0.5 {
                    let chunk = state.g.render(&state.sb, &mut state.fb, &mut state.host);
                    let chunk = chunk.unwrap();
                    let stream_data = chunk.clone().interleave();
                    device.queue(&stream_data);
                    millis_generated += buffer_len as f32 / proj_sr as f32 * 1000.0;
                    state.fb.set_time_to_next_block();
                }
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[derive(PartialEq)]
pub enum ThreadMsg{
    None, Ready, Quit, Refresh, Render, Normalize, Play, Pause, Stop, Skip, Prev, Set(usize), Get, NormVals
}


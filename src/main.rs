use std::fs::File;
use std::io::{ Read, Cursor };
use std::thread;
use std::sync::mpsc;
use std::time::{ Duration, Instant };

use mlua::prelude::*;
use skim::prelude::*;
use sdl2::audio::AudioSpecDesired;
use lv2hm::Lv2Host;
use term_basics_linux as tbl;

mod sample;
mod graph;
mod floww;
mod extensions;
mod adsr;
mod synth;
mod config;
mod state;

use sample::*;
use graph::*;
use floww::*;
use config::*;
use state::*;

fn main() -> Result<(), String>{
    let config = Config::read("project.toml");

    println!("TermDaw: loading \"{}\" with \n\tbuffer_length = {} \n\tproject_samplerate = {} \n\tmain = \"{}\"",
        config.project.name(),
        config.settings.buffer_length(),
        config.settings.project_samplerate(),
        config.settings.main);

    let mut file = File::open(&config.settings.main).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    std::mem::drop(file);

    let proj_sr = config.settings.project_samplerate();
    let buffer_len = config.settings.buffer_length();

    let mut state = State{
        lua: Lua::new(),
        sb: SampleBank::new(proj_sr),
        g: Graph::new(config.settings.buffer_length(), proj_sr),
        host: Lv2Host::new(1000, buffer_len * 2), // acount for l/r
        fb: FlowwBank::new(proj_sr, buffer_len),
        contents,
        config,
        cs: 0,
        render_sr: 48000,
        bd: 16,
        output_vertex: String::new(),
        output_file: String::from("outp.wav"),
        cur_samples: Vec::new(),
        cur_lv2plugins: Vec::new(),
        cur_lv2params: Vec::new(),
    };
    state.refresh(true)?;

    let sdl_context = sdl2::init()?;
    let audio_subsystem = sdl_context.audio()?;
    let desired_spec = AudioSpecDesired {
        freq: Some(proj_sr as i32),
        channels: Some(2),
        samples: None,
    };
    let device = audio_subsystem.open_queue::<f32, _>(None, &desired_spec)?;
    let mut playing = false;
    let mut since = Instant::now();
    let mut millis_generated = 0f32;

    let (transmit_to_ui, receive_in_ui) = mpsc::channel();
    let (transmit_to_main, receive_in_main) = mpsc::channel();

    thread::spawn(move || {
        let options = SkimOptionsBuilder::default()
            .height(Some("8%")).build().unwrap();
        let input = "quit\nrender\nrefresh\nnormalize\nplay\npause\nstop\n>skip\n<prev\nset\nget".to_string();
        let item_reader = SkimItemReader::default();
        loop{
            let items = item_reader.of_bufread(Cursor::new(input.clone()));
            let selected_items = Skim::run_with(&options, Some(items))
                .map(|out| out.selected_items)
                .unwrap_or_else(Vec::new);

            if let Some(item) = selected_items.get(0){
                let command = item.output();
                println!("---- {}", command);
                let tmsg = if command == "quit" { ThreadMsg::Quit }
                else if command == "refresh" { ThreadMsg::Refresh }
                else if command == "render" { ThreadMsg::Render }
                else if command == "normalize" { ThreadMsg::Normalize }
                else if command == "play" { ThreadMsg::Play }
                else if command == "pause" { ThreadMsg::Pause }
                else if command == "stop" { ThreadMsg::Stop }
                else if command == ">skip" { ThreadMsg::Skip }
                else if command == "<prev" { ThreadMsg::Prev }
                else if command == "set" {
                    let raw = tbl::input_field();
                    let time: Option<f32> = tbl::string_to_value(&raw);
                    if let Some(float) = time{
                        if float >= 0.0{
                            let t = (float * proj_sr as f32) as usize;
                            ThreadMsg::Set(t)
                        } else {
                            println!("Error: time needs to be positive.");
                            ThreadMsg::None
                        }
                    } else {
                        println!("Error: could not parse time, did not set time.");
                        ThreadMsg::None
                    }
                }
                else if command == "get" { ThreadMsg::Get }
                else { ThreadMsg::None };
                transmit_to_main.send(tmsg).unwrap();
            } else {
                println!("TermDaw: command not found!");
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
            match rec{
                ThreadMsg::Quit => {
                    break;
                },
                ThreadMsg::Refresh => {
                    state.refresh(false)?;
                    playing = false;
                },
                ThreadMsg::Render => {
                    state.render();
                    playing = false;
                },
                ThreadMsg::Normalize => {
                    state.scan();
                    playing = false;
                },
                ThreadMsg::Play => {
                    playing = true;
                    since = Instant::now();
                    millis_generated = 0.0;
                    device.resume();
                },
                ThreadMsg::Pause => {
                    playing = false;
                    device.pause();
                },
                ThreadMsg::Stop => {
                    playing = false;
                    device.pause();
                    device.clear();
                    state.g.set_time(0);
                    state.fb.set_time(0);
                },
                ThreadMsg::Skip => {
                    device.pause();
                    device.clear();
                    let time = state.g.change_time(5 * proj_sr, true);
                    state.fb.set_time(time);
                    device.resume();
                }
                ThreadMsg::Prev => {
                    device.pause();
                    device.clear();
                    let time = state.g.change_time(5 * proj_sr, false);
                    state.fb.set_time(time);
                    device.resume();
                }
                ThreadMsg::Set(time) => {
                    device.pause();
                    device.clear();
                    state.g.set_time(time);
                    state.fb.set_time(time);
                    device.resume();
                }
                ThreadMsg::Get => {
                    let t = state.g.get_time();
                    let tf = t as f32 / proj_sr as f32;
                    println!("Frame: {}, Time: {}", t, tf);
                }
                _ => {}
            }
            transmit_to_ui.send(ThreadMsg::Ready).unwrap();
        }
        if playing{
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
        std::thread::sleep(Duration::from_millis(10));
    }

    Ok(())
}

#[derive(PartialEq)]
pub enum ThreadMsg{
    None, Ready, Quit, Refresh, Render, Normalize, Play, Pause, Stop, Skip, Prev, Set(usize), Get
}


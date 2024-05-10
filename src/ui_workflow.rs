use crate::state::*;

use std::{
    thread,
    sync::mpsc,
    time::{ Duration, Instant },
    io::Cursor,
};

use term_basics_linux::*;
use skim::prelude::*;

pub fn run_ui_workflow(
    proj_sr: usize, buffer_len: usize, state: State, device: sdl2::audio::AudioQueue<f32>
){
    let (transmit_to_ui, receive_in_ui) = mpsc::channel();
    let (transmit_to_main, receive_in_main) = mpsc::channel();

    launch_ui_thread(proj_sr, transmit_to_main, receive_in_ui);
    ui_partner(state, device, proj_sr, buffer_len, transmit_to_ui, receive_in_main);
}

#[derive(PartialEq)]
enum UiThreadMsg{
    None, Ready, Quit, Refresh, Render, Normalize, Play, Pause, Stop, Skip, Prev, Set(usize),
    Get, NormVals
}

fn launch_ui_thread(
    proj_sr: usize, transmit_to_main: mpsc::Sender<UiThreadMsg>,
    receive_in_ui: mpsc::Receiver<UiThreadMsg>
){
    thread::spawn(move || {
        let options = SkimOptionsBuilder::default()
            .height(Some("8%")).build().unwrap();
        let input =
            "quit\nrender\nrefresh\nnormalize\nplay\npause\nstop\n>skip\n<prev\nset\nget\nnorm-vals"
            .to_string();
        let item_reader = SkimItemReader::default();
        loop{
            let items = item_reader.of_bufread(Cursor::new(input.clone()));
            let selected_items = Skim::run_with(&options, Some(items))
                .map(|out| out.selected_items)
                .unwrap_or_default();

            if let Some(item) = selected_items.first(){
                let command = item.output();
                println!("{}---- {}", UC::Magenta, command);
                let tmsg = if command == "quit" { UiThreadMsg::Quit }
                else if command == "refresh" { UiThreadMsg::Refresh }
                else if command == "render" { UiThreadMsg::Render }
                else if command == "normalize" { UiThreadMsg::Normalize }
                else if command == "play" { UiThreadMsg::Play }
                else if command == "pause" { UiThreadMsg::Pause }
                else if command == "stop" { UiThreadMsg::Stop }
                else if command == ">skip" { UiThreadMsg::Skip }
                else if command == "<prev" { UiThreadMsg::Prev }
                else if command == "norm-vals" { UiThreadMsg::NormVals }
                else if command == "set" {
                    let raw = input_field();
                    let time: Option<f32> = string_to_value(&raw);
                    if let Some(float) = time{
                        if float >= 0.0{
                            let t = (float * proj_sr as f32) as usize;
                            UiThreadMsg::Set(t)
                        } else {
                            println!("{}Error: time needs to be positive.", UC::Red);
                            UiThreadMsg::None
                        }
                    } else {
                        println!("{}Error: could not parse time, did not set time.", UC::Red);
                        UiThreadMsg::None
                    }
                }
                else if command == "get" { UiThreadMsg::Get }
                else { UiThreadMsg::None };
                transmit_to_main.send(tmsg).unwrap();
            } else {
                println!("{}TermDaw: command not found!", UC::Red);
                continue;
            }
            for received in &receive_in_ui{
                if received == UiThreadMsg::Ready{
                    break;
                }
            }
        }
    });
}

fn ui_partner(
    mut state: State, device: sdl2::audio::AudioQueue<f32>, proj_sr: usize, buffer_len: usize,
    transmit_to_ui: mpsc::Sender<UiThreadMsg>, receive_in_main: mpsc::Receiver<UiThreadMsg>
){
    let mut playing = false;
    let mut since = Instant::now();
    let mut millis_generated = 0f32;
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
                UiThreadMsg::Quit => {
                    break;
                },
                UiThreadMsg::Refresh => {
                    state.refresh();
                    playing = false;
                    device.clear();
                    device.pause();
                },
                UiThreadMsg::Render => {
                    check_loaded!({
                        device.clear();
                        device.pause();
                        playing = false;
                        state.render();
                    });
                },
                UiThreadMsg::Normalize => {
                    check_loaded!({
                        device.clear();
                        device.pause();
                        playing = false;
                        state.scan_exact();
                    });
                },
                UiThreadMsg::Play => {
                    check_loaded!({
                        playing = true;
                        since = Instant::now();
                        millis_generated = 0.0;
                        device.resume();
                    });
                },
                UiThreadMsg::Pause => {
                    playing = false;
                    device.pause();
                },
                UiThreadMsg::Stop => {
                    check_loaded!({
                        playing = false;
                        device.pause();
                        device.clear();
                        state.g.set_time(0);
                        state.fb.set_time(0);
                    });
                },
                UiThreadMsg::Skip => {
                    check_loaded!({
                        device.clear();
                        let time = state.g.change_time(5 * proj_sr, true);
                        state.fb.set_time(time);
                    });
                }
                UiThreadMsg::Prev => {
                    check_loaded!({
                        device.clear();
                        let time = state.g.change_time(5 * proj_sr, false);
                        state.fb.set_time(time);
                    });
                }
                UiThreadMsg::Set(time) => {
                    check_loaded!({
                        device.clear();
                        state.g.set_time(time);
                        state.fb.set_time(time);
                    });
                }
                UiThreadMsg::Get => {
                    check_loaded!({
                        let t = state.g.get_time();
                        let tf = t as f32 / proj_sr as f32;
                        println!("{s}Frame: {b}{t}{s}, Time: {b}{tf}",
                            s = UC::Std, b = UC::Blue, t = t, tf = tf);
                    });
                }
                UiThreadMsg::NormVals => {
                    check_loaded!({
                        state.g.print_normalization_values();
                    });
                }
                _ => {}
            }
            transmit_to_ui.send(UiThreadMsg::Ready).unwrap();
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

use std::fs::File;
use std::io::{ Read, Cursor };
use std::thread;
use std::sync::mpsc;
use std::time::{ Duration, Instant };

use mlua::prelude::*;
use rubato::{ Resampler, SincFixedIn, InterpolationType, InterpolationParameters, WindowFunction };
use serde::Deserialize;
use skim::prelude::*;
use sdl2::audio::AudioSpecDesired;
use lv2hm::Lv2Host;

mod sample;
mod graph;
mod floww;
use sample::*;
use graph::*;
use floww::*;

use apres::MIDI;
use apres::MIDIEvent::{NoteOff, NoteOn};

fn main() -> Result<(), String>{
    let mut file = File::open("project.toml").unwrap();

    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    let config: Config = toml::from_str(&contents).unwrap();
    std::mem::drop(file);

    println!("TermDaw: loading \"{}\" with \n\tbuffer_length = {} \n\tproject_samplerate = {} \n\tmain = \"{}\"",
        config.project.name,
        config.settings.buffer_length,
        config.settings.project_samplerate,
        config.settings.main);

    let mut file = File::open(&config.settings.main).unwrap();
    contents.clear();
    file.read_to_string(&mut contents).unwrap();
    std::mem::drop(file);

    let mut state = State{
        lua: Lua::new(),
        sb: SampleBank::new(config.settings.project_samplerate),
        g: Graph::new(config.settings.buffer_length),
        host: Lv2Host::new(1000, config.settings.buffer_length * 2), // acount for l/r
        fb: FlowwBank::new(config.settings.project_samplerate, config.settings.buffer_length),
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
    state.refresh()?;

    let sdl_context = sdl2::init()?;
    let audio_subsystem = sdl_context.audio()?;
    let desired_spec = AudioSpecDesired {
        freq: Some(state.config.settings.project_samplerate as i32),
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
        let input = "quit\nrender\nrefresh\nplay\npause\nstop".to_string();
        let item_reader = SkimItemReader::default();
        loop{
            let items = item_reader.of_bufread(Cursor::new(input.clone()));
            let selected_items = Skim::run_with(&options, Some(items))
                .map(|out| out.selected_items)
                .unwrap_or_else(Vec::new);

            if let Some(item) = selected_items.get(0){
                let command = item.output();
                println!("---- {}", command);
                let tmsg = if command == "quit"{ ThreadMsg::Quit }
                else if command == "refresh"{ ThreadMsg::Refresh }
                else if command == "render"{ ThreadMsg::Render }
                else if command == "play"{ ThreadMsg::Play }
                else if command == "pause"{ ThreadMsg::Pause }
                else if command == "stop" { ThreadMsg::Stop }
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
                    state.refresh()?;
                    playing = false;
                },
                ThreadMsg::Render => {
                    state.render();
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
                let stream_data = chunk.clone().deinterleave();
                device.queue(&stream_data);
                millis_generated += state.config.settings.buffer_length as f32 / state.config.settings.project_samplerate as f32 * 1000.0;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    Ok(())
}

#[derive(PartialEq)]
pub enum ThreadMsg{
    None, Ready, Quit, Refresh, Render, Play, Pause, Stop,
}

struct State{
    lua: Lua,
    sb: SampleBank,
    g: Graph,
    host: Lv2Host,
    fb: FlowwBank,
    config: Config,
    contents: String,
    cs: usize,
    render_sr: usize,
    bd: usize,
    output_vertex: String,
    output_file: String,
    cur_samples: Vec<(String, String)>,
    cur_lv2plugins: Vec<(String, String)>,
    cur_lv2params: Vec<(String, String, f32)>,
}

impl State{
    fn refresh(&mut self) -> Result<(), String>{
        let psr = self.config.settings.project_samplerate;
        let bl = self.config.settings.buffer_length;

        let mut file = File::open(&self.config.settings.main).unwrap();
        self.contents.clear();
        file.read_to_string(&mut self.contents).unwrap();

        let mut new_samples = Vec::new();
        let mut new_sums = Vec::new();
        let mut new_norms = Vec::new();
        let mut new_sampleloops = Vec::new();
        let mut new_edges = Vec::new();
        let mut new_lv2plugins = Vec::new();
        let mut new_lv2params = Vec::new();
        let mut new_lv2fxs = Vec::new();

        let mut cs = self.cs;
        let mut render_sr = self.render_sr;
        let mut bd = self.bd;
        let mut output_file = std::mem::take(&mut self.output_file);
        let mut output_vertex = std::mem::take(&mut self.output_vertex);

        self.lua.scope(|scope| {
            // ---- Settings
            self.lua.globals().set("set_length", scope.create_function_mut(|_, frames: usize| {
                cs = ((psr * frames) as f32 / bl as f32).ceil() as usize;
                Ok(())
            })?)?;
            self.lua.globals().set("set_render_samplerate", scope.create_function_mut(|_, new_sr: usize| {
                render_sr = new_sr;
                Ok(())
            })?)?;
            self.lua.globals().set("set_render_bitdepth", scope.create_function_mut(|_, new_bd: usize| {
                bd = new_bd;
                Ok(())
            })?)?;
            self.lua.globals().set("set_output_file", scope.create_function_mut(|_, out: String| {
                output_file = out;
                Ok(())
            })?)?;
            // ---- Resources
            // load_sample(name, file)
            self.lua.globals().set("load_sample", scope.create_function_mut(|_, seed: (String, String)| {
                new_samples.push(seed);
                Ok(())
            })?)?;
            // load_lv2(name, uri)
            self.lua.globals().set("load_lv2", scope.create_function_mut(|_, seed: (String, String)| {
                new_lv2plugins.push(seed);
                Ok(())
            })?)?;
            // parameter(plugin, name, value)
            self.lua.globals().set("parameter", scope.create_function_mut(|_, seed: (String, String, f32)| {
                new_lv2params.push(seed);
                Ok(())
            })?)?;
            // ---- Graph
            // add_sum(name, gain, angle)
            self.lua.globals().set("add_sum", scope.create_function_mut(|_, seed: (String, f32, f32)| {
                new_sums.push(seed);
                Ok(())
            })?)?;
            // add_normalize(name, gain, angle)
            self.lua.globals().set("add_normalize", scope.create_function_mut(|_, seed: (String, f32, f32)| {
                new_norms.push(seed);
                Ok(())
            })?)?;
            // add_sampleloop(name, gain, angle, sample)
            self.lua.globals().set("add_sampleloop", scope.create_function_mut(|_, seed: (String, f32, f32, String)| {
                new_sampleloops.push(seed);
                Ok(())
            })?)?;
            // add_lv2fx(name, gain, angle, plugin)
            self.lua.globals().set("add_lv2fx", scope.create_function_mut(|_, seed: (String, f32, f32, String)| {
                new_lv2fxs.push(seed);
                Ok(())
            })?)?;
            // connect(name, name)
            self.lua.globals().set("connect", scope.create_function_mut(|_, seed: (String, String)| {
                new_edges.push(seed);
                Ok(())
            })?)?;
            self.lua.globals().set("set_output", scope.create_function_mut(|_, out: String| {
                output_vertex = out;
                Ok(())
            })?)?;
            self.lua.load(&self.contents).exec()
        }).unwrap();

        self.cs = cs;
        self.bd = bd;
        self.render_sr = render_sr;
        self.output_file = output_file;
        self.output_vertex = output_vertex;

        fn diff<T: PartialEq + Clone>(old: &[T], new: &[T]) -> (Vec<T>, Vec<T>){
            let mut adds = Vec::new();
            for t in new{
                if !old.contains(t){
                    adds.push(t.clone());
                }
            }
            let mut removes = Vec::new();
            for t in old{
                if !new.contains(t){
                    removes.push(t.clone());
                }
            }
            (adds, removes)
        }

        // samples may be long, try not to reallocate to much shit
        let (pos, neg) = diff(&self.cur_samples, &new_samples);
        for (name, _) in neg {
            println!("Info: sample \"{}\" will be removed from the sample bank.", name);
            self.sb.mark_dead(&name);
        }
        println!("Status: refreshing sample bank.");
        self.sb.refresh();
        for (name, file) in pos {
            println!("Status: adding sample \"{}\" to the sample bank.", name);
            self.sb.add(name, &file)?;
        }
        // same for plugins
        // TODO: make renaming possible
        let (pos, neg) = diff(&self.cur_lv2plugins, &new_lv2plugins);
        for (name, _) in neg { // TODO: make plugins removable
            self.host.remove_plugin(&name);
        }
        for (name, uri) in pos {
            self.host.add_plugin(&uri, name.clone(), std::ptr::null_mut()).unwrap_or_else(|_| panic!("Error: Lv2hm could not add plugin with uri {}.", uri));
            println!("Info: added plugin {} with uri {}.", name, uri);
        }

        // need diff to see what params we need to reset
        let (pos, neg) = diff(&self.cur_lv2params, &new_lv2params);
        for (plugin, name, _) in neg { // TODO: make params resetable in Lv2hm
            self.host.reset_value(&plugin, &name);
        }
        for (plugin, name, value) in pos{
            self.host.set_value(&plugin, &name, value);
        }

        // just rebuild the damn thing, if it becomes problamatic i'll do something about it,
        // probably :)
        println!("Status: rebuilding graph.");
        self.g.reset();
        for (name, gain, angle) in &new_sums { self.g.add(Vertex::new(bl, *gain, *angle, VertexExt::sum()), name.to_owned()); }
        for (name, gain, angle) in &new_norms { self.g.add(Vertex::new(bl, *gain, *angle, VertexExt::normalize()), name.to_owned()); }
        for (name, gain, angle, sample) in &new_sampleloops { self.g.add(Vertex::new(bl, *gain, *angle, VertexExt::sample_loop(self.sb.get_index(&sample).unwrap())), name.to_owned()); }
        for (name, gain, angle, plugin) in &new_lv2fxs { self.g.add(Vertex::new(bl, *gain, *angle, VertexExt::lv2fx(self.host.get_index(plugin).unwrap())), name.to_owned()); }
        for (a, b) in &new_edges { self.g.connect(a, b); }

        self.g.set_output(&self.output_vertex);
        if !self.g.check_graph(){
            return Err("TermDaw: graph check failed.".to_owned());
        }
        self.g.scan(&self.sb, &mut self.fb, &mut self.host, self.cs);

        self.cur_samples = new_samples;
        self.cur_lv2plugins = new_lv2plugins;
        self.cur_lv2params = new_lv2params;

        println!("Status: refreshed.");
        Ok(())
    }

    fn render(&mut self) {
        println!("Status: started rendering");
        let psr = self.config.settings.project_samplerate;
        let bl = self.config.settings.buffer_length;

        let (msr, mbd) = self.sb.get_max_sr_bd();
        if psr > self.render_sr{
            println!("TermDaw: warning: render will down sample from {}(project s.r.) to {}.", psr, self.render_sr);
        }
        if msr > self.render_sr{
            println!("TermDaw: warning: render will down sample from peak input quality({}) to {}.", msr, self.render_sr);
        }
        if !(self.bd == 8 || self.bd == 16 || self.bd == 24 || self.bd == 32) {
            panic!("Bitdepth of {} not supported: choose bitdepth in {{8, 16, 24, 32}}.", self.bd);
        }
        if mbd > self.bd{
            println!("TermDaw: warning: render will lose bitdepth from peak input quality({} bits) to {} bits", mbd, self.bd);
        }
        let spec = hound::WavSpec{
            channels: 2,
            sample_rate: self.render_sr as u32,
            bits_per_sample: self.bd as u16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(self.output_file.clone(), spec).unwrap();
        let amplitude = if self.bd < 32 { ((1 << (self.bd - 1)) - 1) as f32 }
        else { i32::MAX as f32 };
        fn write_16s<T: std::io::Write + std::io::Seek>(writer: &mut hound::WavWriter<T>, l: &[f32], r: &[f32], len: usize, amplitude: f32){
            for i in 0..len{
                writer.write_sample((l[i] * amplitude) as i16).unwrap();
                writer.write_sample((r[i] * amplitude) as i16).unwrap();
            }
        }
        fn write_32s<T: std::io::Write + std::io::Seek>(writer: &mut hound::WavWriter<T>, l: &[f32], r: &[f32], len: usize, amplitude: f32){
            for i in 0..len{
                writer.write_sample((l[i] * amplitude) as i32).unwrap();
                writer.write_sample((r[i] * amplitude) as i32).unwrap();
            }
        }
        if psr > self.render_sr{
            let params = InterpolationParameters {
                sinc_len: 256,
                f_cutoff: 0.95,
                interpolation: InterpolationType::Nearest,
                oversampling_factor: 160,
                window: WindowFunction::BlackmanHarris2,
            };
            let mut resampler = SincFixedIn::<f32>::new(
                self.render_sr as f64 / psr as f64,
                params, bl, 2
            );
            for _ in 0..self.cs{
                let chunk = self.g.render(&self.sb, &mut self.fb, &mut self.host);
                if chunk.is_none() { continue; }
                let chunk = chunk.unwrap();
                let waves_in = vec![chunk.l.clone(), chunk.r.clone()];
                let waves_out = resampler.process(&waves_in).unwrap();
                if self.bd > 16 { write_32s(&mut writer, &waves_out[0], &waves_out[1], waves_out[0].len(), amplitude); }
                else { write_16s(&mut writer, &waves_out[0], &waves_out[1], waves_out[0].len(), amplitude); }
            }
        } else {
            for _ in 0..self.cs{
                let chunk = self.g.render(&self.sb, &mut self.fb, &mut self.host);
                if chunk.is_none() { continue; }
                let chunk = chunk.unwrap();
                if self.bd > 16 { write_32s(&mut writer, &chunk.l, &chunk.r, chunk.len(), amplitude); }
                else { write_16s(&mut writer, &chunk.l, &chunk.r, chunk.len(), amplitude); }
            }
        }
        self.g.set_time(0);
        println!("Status: done rendering.");
    }
}

#[derive(Deserialize)]
struct Config{
    project: Project,
    settings: Settings,
}

#[derive(Deserialize)]
struct Project{
    name: String,
}

#[derive(Deserialize)]
struct Settings{
    buffer_length: usize,
    project_samplerate: usize,
    main: String,
}


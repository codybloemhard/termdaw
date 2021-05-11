use std::fs::File;
use std::io::prelude::*;

use mlua::prelude::*;
use rubato::{ Resampler, SincFixedIn, InterpolationType, InterpolationParameters, WindowFunction };
use serde::Deserialize;

mod sample;
mod graph;
use sample::*;
use graph::*;

fn main() -> Result<(), String>{
    let mut file = File::open("project.toml").unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    let config: Config = toml::from_str(&contents).unwrap();
    std::mem::drop(file);

    let name = config.project.name;
    let main = config.settings.main;
    let bl = config.settings.buffer_length;
    let psr = config.settings.project_samplerate;
    println!("TermDaw: loading \"{}\" with \n\tbuffer_length = {} \n\tproject_samplerate = {} \n\tmain = \"{}\"", name, bl, psr, main);

    let mut file = File::open(&main).unwrap();
    contents.clear();
    file.read_to_string(&mut contents).unwrap();
    std::mem::drop(file);

    let lua = Lua::new();

    let mut cs = 0;
    let mut render_sr = 48000;
    let mut bd = 16;
    let mut output_vertex = String::new();
    let mut output_file = String::from("outp.wav");

    let mut sb = SampleBank::new(psr);
    let mut g = Graph::new(bl);

    let mut samples_to_load = Vec::new();
    let mut seeds_sum = Vec::new();
    let mut seeds_norm = Vec::new();
    let mut seeds_sl = Vec::new();
    let mut connections = Vec::new();

    lua.scope(|scope| {
        // ---- Settings
        lua.globals().set("set_length", scope.create_function_mut(|_, frames: usize| {
            cs = ((psr * frames) as f32 / bl as f32).ceil() as usize;
            Ok(())
        })?)?;
        lua.globals().set("set_render_samplerate", scope.create_function_mut(|_, new_sr: usize| {
            render_sr = new_sr;
            Ok(())
        })?)?;
        lua.globals().set("set_render_bitdepth", scope.create_function_mut(|_, new_bd: usize| {
            bd = new_bd;
            Ok(())
        })?)?;
        lua.globals().set("set_output_file", scope.create_function_mut(|_, out: String| {
            output_file = out;
            Ok(())
        })?)?;
        // ---- Resources
        // load_sample(name, file)
        lua.globals().set("load_sample", scope.create_function_mut(|_, seed: (String, String)| {
            samples_to_load.push(seed);
            Ok(())
        })?)?;
        // ---- Graph
        // add_sum(name, gain, angle)
        lua.globals().set("add_sum", scope.create_function_mut(|_, seed: (String, f32, f32)| {
            seeds_sum.push(seed);
            Ok(())
        })?)?;
        // add_normalize(name, gain, angle)
        lua.globals().set("add_normalize", scope.create_function_mut(|_, seed: (String, f32, f32)| {
            seeds_norm.push(seed);
            Ok(())
        })?)?;
        // add_sampleloop(name, gain, angle, sample)
        lua.globals().set("add_sampleloop", scope.create_function_mut(|_, seed: (String, f32, f32, String)| {
            seeds_sl.push(seed);
            Ok(())
        })?)?;
        // connect(name, name)
        lua.globals().set("connect", scope.create_function_mut(|_, seed: (String, String)| {
            connections.push(seed);
            Ok(())
        })?)?;
        lua.globals().set("set_output", scope.create_function_mut(|_, out: String| {
            output_vertex = out;
            Ok(())
        })?)?;
        lua.load(&contents).exec()
    }).unwrap();

    for (name, file) in samples_to_load { sb.add(name, &file)?; }
    for (name, gain, angle) in seeds_sum { g.add(Vertex::new(bl, gain, angle, VertexExt::sum()), name); }
    for (name, gain, angle) in seeds_norm { g.add(Vertex::new(bl, gain, angle, VertexExt::normalize()), name); }
    for (name, gain, angle, sample) in seeds_sl { g.add(Vertex::new(bl, gain, angle, VertexExt::sample_loop(sb.get(&sample).unwrap())), name); }
    for (a, b) in connections { g.connect(&a, &b); }

    g.set_output(&output_vertex);
    if !g.check_graph(){
        return Err("TermDaw: graph check failed.".to_owned());
    }
    g.scan(cs);

    render((psr, render_sr, bd, bl, cs), &output_file, &sb, &mut g);
    Ok(())
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

fn render((psr, render_sr, bd, bl, cs): (usize, usize, usize, usize, usize), output_file: &str, sb: &SampleBank, g: &mut Graph) {
    let (msr, mbd) = sb.get_max_sr_bd();
    if psr > render_sr{
        println!("TermDaw: warning: render will down sample from {}(project s.r.) to {}.", psr, render_sr);
    }
    if msr > render_sr{
        println!("TermDaw: warning: render will down sample from peak input quality({}) to {}.", msr, render_sr);
    }
    if !(bd == 8 || bd == 16 || bd == 24 || bd == 32) { panic!("Bitdepth of {} not supported: choose bitdepth in {{8, 16, 24, 32}}.", bd); }
    if mbd > bd{
        println!("TermDaw: warning: render will lose bitdepth from peak input quality({} bits) to {} bits", mbd, bd);
    }
    let spec = hound::WavSpec{
        channels: 2,
        sample_rate: render_sr as u32,
        bits_per_sample: bd as u16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(output_file, spec).unwrap();
    let amplitude = if bd < 32 { ((1 << (bd - 1)) - 1) as f32 }
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
    if psr > render_sr{
        let params = InterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: InterpolationType::Nearest,
            oversampling_factor: 160,
            window: WindowFunction::BlackmanHarris2,
        };
        let mut resampler = SincFixedIn::<f32>::new(
            render_sr as f64 / psr as f64,
            params, bl, 2
        );
        for _ in 0..cs{
            let chunk = g.render();
            if chunk.is_none() { continue; }
            let chunk = chunk.unwrap();
            let waves_in = vec![chunk.l.clone(), chunk.r.clone()];
            let waves_out = resampler.process(&waves_in).unwrap();
            if bd > 16 { write_32s(&mut writer, &waves_out[0], &waves_out[1], waves_out[0].len(), amplitude); }
            else { write_16s(&mut writer, &waves_out[0], &waves_out[1], waves_out[0].len(), amplitude); }
        }
    } else {
        for _ in 0..cs{
            let chunk = g.render();
            if chunk.is_none() { continue; }
            let chunk = chunk.unwrap();
            if bd > 16 { write_32s(&mut writer, &chunk.l, &chunk.r, chunk.len(), amplitude); }
            else { write_16s(&mut writer, &chunk.l, &chunk.r, chunk.len(), amplitude); }
        }
    }
}

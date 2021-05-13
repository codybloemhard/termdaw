use std::fs::File;
use std::io::{ self, Read };

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

    let mut cur_samples = Vec::new();

    loop{
        let mut buffer = String::new();
        let stdin = io::stdin();
        stdin.read_line(&mut buffer).unwrap();
        println!("---- {}", buffer);

        let mut file = File::open(&main).unwrap();
        contents.clear();
        file.read_to_string(&mut contents).unwrap();
        std::mem::drop(file);

        let mut new_samples = Vec::new();
        let mut new_sums = Vec::new();
        let mut new_norms = Vec::new();
        let mut new_sampleloops = Vec::new();
        let mut new_edges = Vec::new();

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
                new_samples.push(seed);
                Ok(())
            })?)?;
            // ---- Graph
            // add_sum(name, gain, angle)
            lua.globals().set("add_sum", scope.create_function_mut(|_, seed: (String, f32, f32)| {
                new_sums.push(seed);
                Ok(())
            })?)?;
            // add_normalize(name, gain, angle)
            lua.globals().set("add_normalize", scope.create_function_mut(|_, seed: (String, f32, f32)| {
                new_norms.push(seed);
                Ok(())
            })?)?;
            // add_sampleloop(name, gain, angle, sample)
            lua.globals().set("add_sampleloop", scope.create_function_mut(|_, seed: (String, f32, f32, String)| {
                new_sampleloops.push(seed);
                Ok(())
            })?)?;
            // connect(name, name)
            lua.globals().set("connect", scope.create_function_mut(|_, seed: (String, String)| {
                new_edges.push(seed);
                Ok(())
            })?)?;
            lua.globals().set("set_output", scope.create_function_mut(|_, out: String| {
                output_vertex = out;
                Ok(())
            })?)?;
            lua.load(&contents).exec()
        }).unwrap();

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
        let (pos, neg) = diff(&cur_samples, &new_samples);
        for (name, _) in neg {
            println!("Info: sample \"{}\" will be removed from the sample bank.", name);
            sb.mark_dead(&name);
        }
        println!("Status: refreshing sample bank.");
        sb.refresh();
        for (name, file) in pos {
            println!("Status: adding sample \"{}\" to the sample bank.", name);
            sb.add(name, &file)?;
        }

        // just rebuild the damn thing, if it becomes problamatic i'll do something about it,
        // probably :)
        println!("Status: rebuilding graph.");
        g.reset();
        for (name, gain, angle) in &new_sums { g.add(Vertex::new(bl, *gain, *angle, VertexExt::sum()), name.to_owned()); }
        for (name, gain, angle) in &new_norms { g.add(Vertex::new(bl, *gain, *angle, VertexExt::normalize()), name.to_owned()); }
        for (name, gain, angle, sample) in &new_sampleloops { g.add(Vertex::new(bl, *gain, *angle, VertexExt::sample_loop(sb.get_index(&sample).unwrap())), name.to_owned()); }
        for (a, b) in &new_edges { g.connect(a, b); }

        g.set_output(&output_vertex);
        if !g.check_graph(){
            return Err("TermDaw: graph check failed.".to_owned());
        }
        g.scan(&sb, cs);

        cur_samples = new_samples;
        println!("Status: refreshed.");

        render((psr, render_sr, bd, bl, cs), &output_file, &sb, &mut g);
    }

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
    println!("Status: started rendering");
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
            let chunk = g.render(sb);
            if chunk.is_none() { continue; }
            let chunk = chunk.unwrap();
            let waves_in = vec![chunk.l.clone(), chunk.r.clone()];
            let waves_out = resampler.process(&waves_in).unwrap();
            if bd > 16 { write_32s(&mut writer, &waves_out[0], &waves_out[1], waves_out[0].len(), amplitude); }
            else { write_16s(&mut writer, &waves_out[0], &waves_out[1], waves_out[0].len(), amplitude); }
        }
    } else {
        for _ in 0..cs{
            let chunk = g.render(sb);
            if chunk.is_none() { continue; }
            let chunk = chunk.unwrap();
            if bd > 16 { write_32s(&mut writer, &chunk.l, &chunk.r, chunk.len(), amplitude); }
            else { write_16s(&mut writer, &chunk.l, &chunk.r, chunk.len(), amplitude); }
        }
    }
    println!("Status: done rendering.");
}

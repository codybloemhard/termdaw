use crate::extensions::*;
use crate::synth::*;
use crate::adsr::*;
use crate::graph::*;
use crate::floww::*;
use crate::config::*;
use crate::sample::*;

use rubato::{ Resampler, SincFixedIn, InterpolationType, InterpolationParameters, WindowFunction };
use fnrs::{ vecs };
use mlua::prelude::*;
use lv2hm::Lv2Host;

use std::fs::File;
use std::io::Read;

pub struct State{
    pub lua: Lua,
    pub sb: SampleBank,
    pub g: Graph,
    pub host: Lv2Host,
    pub fb: FlowwBank,
    pub config: Config,
    pub contents: String,
    pub cs: usize,
    pub render_sr: usize,
    pub bd: usize,
    pub output_vertex: String,
    pub output_file: String,
    pub cur_samples: Vec<(String, String)>,
    pub cur_lv2plugins: Vec<(String, String)>,
    pub cur_lv2params: Vec<(String, String, f32)>,
}

impl State{
    pub fn refresh(&mut self, first: bool) -> Result<(), String>{
        let psr = self.config.settings.project_samplerate();
        let bl = self.config.settings.buffer_length();

        let mut file = File::open(&self.config.settings.main).unwrap();
        self.contents.clear();
        file.read_to_string(&mut self.contents).unwrap();

        vecs!(
            new_samples, new_lv2plugins, new_lv2params, midis,
            sums, norms, sampleloops, samplemultis, samplelerps, debugsines, synths, lv2fxs, adsrs,
            edges
        );

        let mut cs = self.cs;
        let mut render_sr = self.render_sr;
        let mut bd = self.bd;
        let mut output_file = std::mem::take(&mut self.output_file);
        let mut output_vertex = std::mem::take(&mut self.output_vertex);

        self.lua.scope(|scope| {
            // ---- Macros
            macro_rules! seed{
                ($name:expr, $stype:ty, $vec:ident) => {
                    self.lua.globals().set($name, scope.create_function_mut(|_, seed: $stype| {
                        $vec.push(seed);
                        Ok(())
                    })?)?;
                };
            }
            macro_rules! setter{
                ($name:expr, $stype:ty, $var:ident) => {
                    self.lua.globals().set($name, scope.create_function_mut(|_, arg: $stype| {
                        $var = arg;
                        Ok(())
                    })?)?;
                };
            }
            // ---- Settings
            self.lua.globals().set("set_length", scope.create_function_mut(|_, seconds: f32| {
                cs = (psr as f32 * seconds / bl as f32).ceil() as usize;
                Ok(())
            })?)?;
            setter!("set_render_samplerate", usize, render_sr);
            setter!("set_render_bitdepth", usize, bd);
            setter!("set_output_file", String, output_file);
            // ---- Resources
                // load_sample(name, file)
            seed!("load_sample", (String, String), new_samples);
                // load_midi(name, file)
            seed!("load_midi_floww", (String, String), midis);
                // load_lv2(name, uri)
            seed!("load_lv2", (String, String), new_lv2plugins);
                // parameter(plugin, name, value)
            seed!("parameter", (String, String, f32), new_lv2params);
            // ---- Graph
                // add_sum(name, gain, angle)
            seed!("add_sum", (String, f32, f32), sums);
                // add_normalize(name, gain, angle)
            seed!("add_normalize", (String, f32, f32), norms);
                // add_sampleloop(name, gain, angle, sample)
            seed!("add_sampleloop", (String, f32, f32, String), sampleloops);
                // add_sample_multi(name, gain, angle, sample, floww, note)
            seed!("add_sample_multi", (String, f32, f32, String, String, i32), samplemultis);
                // add_sample_lerp(name, gain, angle, sample, floww, note, lerp_len)
            seed!("add_sample_lerp", (String, f32, f32, String, String, i32, i32), samplelerps);
                // add_debug_sine(name, gain, angle, floww)
            seed!("add_debug_sine", (String, f32, f32, String), debugsines);
                // add_synth(name, gain, angle, floww, square_vel, square_z, square_adsr_conf,
                //  topflat_vel, topflat_z, topflat_adsr_conf, triangle_vel, triangle_z, triangle_adsr_conf)
            seed!("add_synth", (String, f32, f32, String, f32, f32, Vec<f32>, f32, f32, Vec<f32>, f32, Vec<f32>), synths);
                // add_lv2fx(name, gain, angle, wet, plugin)
            seed!("add_lv2fx", (String, f32, f32, f32, String), lv2fxs);
                // add_adsr(name, gain, angle, wet, floww, use_off, note, adsr_conf)
            seed!("add_adsr", (String, f32, f32, f32, String, bool, i32, Vec<f32>), adsrs);
                // connect(name, name)
            seed!("connect", (String, String), edges);
            // ---- Output
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
        // Just reload all midi, so you can easily import newly inplace generated files
        self.fb.reset();
        for (name, file) in midis{
            self.fb.add_floww(name, &file);
        }
        // Also don't recreate plugins
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

        // just rebuild the damn thing, if it becomes problematic i'll do something about it,
        // probably :)
        println!("Status: rebuilding graph.");
        self.g.reset();
        for (name, gain, angle) in &sums { self.g.add(Vertex::new(bl, *gain, *angle, 0.0, VertexExt::sum()), name.to_owned()); }
        for (name, gain, angle) in &norms { self.g.add(Vertex::new(bl, *gain, *angle, 0.0, VertexExt::normalize()), name.to_owned()); }
        for (name, gain, angle, sample) in &sampleloops { self.g.add(Vertex::new(bl, *gain, *angle, 0.0, VertexExt::sample_loop(self.sb.get_index(sample).unwrap())), name.to_owned()); }
        for (name, gain, angle, sample, floww, note) in &samplemultis {
            let sample = self.sb.get_index(sample).unwrap();
            let floww = self.fb.get_index(floww).unwrap();
            let note = if note < &0 { None }
            else { Some(*note as usize) };
            self.g.add(Vertex::new(bl, *gain, *angle, 0.0, VertexExt::sample_multi(sample, floww, note)), name.to_owned());
        }
        for (name, gain, angle, sample, floww, note, lerp_len) in &samplelerps {
            let sample = self.sb.get_index(sample).unwrap();
            let floww = self.fb.get_index(floww).unwrap();
            let note = if note < &0 { None }
            else { Some(*note as usize) };
            let lerp_len = (*lerp_len).max(0) as usize;
            self.g.add(Vertex::new(bl, *gain, *angle, 0.0, VertexExt::sample_lerp(sample, floww, note, lerp_len)), name.to_owned());
        }
        for (name, gain, angle, floww) in &debugsines {
            let floww = self.fb.get_index(floww).unwrap();
            self.g.add(Vertex::new(bl, *gain, *angle, 0.0, VertexExt::debug_sine(floww)), name.to_owned());
        }
        for (name, gain, angle, floww, sq_vel, sq_z, sq_arr, tf_vel, tf_z, tf_arr, tr_vel, tr_arr) in &synths {
            let floww = self.fb.get_index(floww).unwrap();
            let parse_adsr_conf = |arr| if let Some(config) = build_adsr_conf(arr){
                config
            } else {
                panic!("ADSR config must have 6 or 9 elements");
            };
            let sq_adsr = parse_adsr_conf(sq_arr);
            let tf_adsr = parse_adsr_conf(tf_arr);
            let tr_adsr = parse_adsr_conf(tr_arr);
            self.g.add(Vertex::new(bl, *gain, *angle, 0.0,
                VertexExt::synth(floww,
                    OscConf::new(*sq_vel, sq_z.max(0.0001), sq_adsr),
                    OscConf::new(*tf_vel, *tf_z, tf_adsr),
                    OscConf::new(*tr_vel, 0.0, tr_adsr))),
                name.to_owned()
            );
        }
        for (name, gain, angle, wet, plugin) in &lv2fxs { self.g.add(Vertex::new(bl, *gain, *angle, *wet, VertexExt::lv2fx(self.host.get_index(plugin).unwrap())), name.to_owned()); }
        for (name, gain, angle, wet, floww, use_off, note, conf_arr) in &adsrs {
            let floww = self.fb.get_index(floww).unwrap();
            let note = if note < &0 { None }
            else { Some(*note as usize) };
            let conf = if let Some(config) = build_adsr_conf(conf_arr){
                config
            } else {
                panic!("ADSR config must have 6 or 9 elements");
            };
            self.g.add(Vertex::new(bl, *gain, *angle, *wet, VertexExt::adsr(*use_off, conf, note, floww)), name.to_owned());
        }

        for (a, b) in &edges { self.g.connect(a, b); }

        self.g.set_output(&self.output_vertex);
        if !self.g.check_graph(){
            return Err("TermDaw: graph check failed.".to_owned());
        }

        if self.config.settings.normalize_on_refresh() || first{
            self.scan();
        }

        self.cur_samples = new_samples;
        self.cur_lv2plugins = new_lv2plugins;
        self.cur_lv2params = new_lv2params;

        println!("Status: refreshed.");
        Ok(())
    }

    pub fn scan(&mut self){
        self.g.scan(&self.sb, &mut self.fb, &mut self.host, self.cs);
    }

    pub fn render(&mut self) {
        println!("Status: started rendering");
        let psr = self.config.settings.project_samplerate();
        let bl = self.config.settings.buffer_length();

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
                self.fb.set_time_to_next_block();
            }
        } else {
            for _ in 0..self.cs{
                let chunk = self.g.render(&self.sb, &mut self.fb, &mut self.host);
                if chunk.is_none() { continue; }
                let chunk = chunk.unwrap();
                if self.bd > 16 { write_32s(&mut writer, &chunk.l, &chunk.r, chunk.len(), amplitude); }
                else { write_16s(&mut writer, &chunk.l, &chunk.r, chunk.len(), amplitude); }
                self.fb.set_time_to_next_block();
            }
        }
        self.g.set_time(0);
        println!("Status: done rendering.");
    }
}


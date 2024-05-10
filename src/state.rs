use crate::extensions::*;
use crate::synth::*;
use crate::adsr::*;
use crate::graph::*;
use crate::floww::*;
use crate::config::*;
use crate::sample::*;
use crate::bufferbank::*;
use crate::lv2::*;

use rubato::{ Resampler, SincFixedIn, InterpolationType, InterpolationParameters, WindowFunction };
use fnrs::{ vecs };
use mlua::prelude::*;
use sampsyn::*;
use term_basics_linux::*;

use std::fs::File;
use std::io::Read;
use std::path::Path;

pub struct State{
    pub lua: Lua,
    pub sb: SampleBank,
    pub g: Graph,
    pub host: Lv2Host,
    pub fb: FlowwBank,
    pub bb: BufferBank,
    pub config: Config,
    pub contents: String,
    pub loaded: bool,
    pub cs: usize,
    pub render_sr: usize,
    pub bd: usize,
    pub output_vertex: String,
    pub output_file: String,
    pub cur_samples: Vec<(String, String, String)>,
    pub cur_resources: Vec<(String, String)>,
    pub cur_lv2plugins: Vec<(String, String)>,
    pub cur_lv2params: Vec<(String, String, f32)>,
    pub wdir: String,
}

impl State{
    pub fn refresh(&mut self) {
        self.loaded = false;
        let psr = self.config.settings.project_samplerate();
        let bl = self.config.settings.buffer_length();

        let mut file = if let Ok(f) = File::open(Path::new(&self.wdir).join(&self.config.settings.main)) { f }
        else {
            println!("{}Can't open main lua file!", UC::Red);
            return;
        };
        self.contents.clear();
        if let Err(e) = file.read_to_string(&mut self.contents){
            println!("{}Could not read main lua file!", UC::Red);
            println!("\t{}", e);
            return;
        }

        vecs!(
            new_samples, new_resources, new_lv2plugins, new_lv2params, midis, streams,
            sums, norms, sampleloops, samplemultis, samplelerps, debugsines, synths, sampsyns, lv2fxs, adsrs,
            bandpasses,
            edges
        );

        let mut cs = self.cs;
        let mut render_sr = self.render_sr;
        let mut bd = self.bd;
        let mut output_file = std::mem::take(&mut self.output_file);
        let mut output_vertex = std::mem::take(&mut self.output_vertex);

        let luares = self.lua.scope(|scope| {
            // ---- Macros
            macro_rules! seed{
                ($name:expr, $stype:ty, $vec:ident) => {
                    #[allow(clippy::type_complexity)]
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
            seed!("load_sample", (String, String, String), new_samples);
                // load_resource(name, file)
            seed!("load_resource", (String, String), new_resources);
                // load_midi(name, file)
            seed!("load_midi_floww", (String, String), midis);
                // declare_stream(name)
            seed!("declare_stream", String, streams);
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
                //  topflat_vel, topflat_z, topflat_adsr_conf, triangle_vel, triangle_adsr_conf)
            seed!("add_synth", (String, f32, f32, String, f32, f32, Vec<f32>, f32, f32, Vec<f32>, f32, Vec<f32>), synths);
                // add_sampsyn(name, gain, angle, floww, adsr_conf, resource)
            seed!("add_sampsyn", (String, f32, f32, String, Vec<f32>, String), sampsyns);
                // add_lv2fx(name, gain, angle, wet, plugin)
            seed!("add_lv2fx", (String, f32, f32, f32, String), lv2fxs);
                // add_adsr(name, gain, angle, wet, floww, use_off, note, adsr_conf)
            seed!("add_adsr", (String, f32, f32, f32, String, bool, bool, i32, Vec<f32>), adsrs);
                // add_bandpass(name, gain, angle, wet, cut_off_hz_low, cut_off_hz_high, pass)
            seed!("add_bandpass", (String, f32, f32, f32, f32, f32, bool), bandpasses);
                // connect(name, name)
            seed!("connect", (String, String), edges);
            // ---- Output
            self.lua.globals().set("set_output", scope.create_function_mut(|_, out: String| {
                output_vertex = out;
                Ok(())
            })?)?;
            self.lua.load(&self.contents).exec()
        });
        if let Err(e) = luares{
            println!("{}Could not execute lua code!", UC::Red);
            println!("\t{}", e);
            return;
        }

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

        macro_rules! do_excluding{
            ($to_exclude:expr, $new:expr, $cur:expr) => {
                if !$to_exclude.is_empty(){
                    for name in $to_exclude{
                        $new.retain(|i| i.0 != name);
                    }
                    $cur = $new;
                    return;
                }
                $cur = $new;
            };
        }

        // samples may be long, try not to reallocate to much shit
        let (pos, neg) = diff(&self.cur_samples, &new_samples);
        for (name, _, _) in neg {
            println!("{s}Info: sample {b}\"{x}\"{s} will be removed from the sample bank.",
                    s = UC::Std, b = UC::Blue, x = name);
            self.sb.mark_dead(&name);
        }
        println!("{}Status: refreshing sample bank.", UC::Std);
        self.sb.refresh();
        let mut to_exclude = Vec::new();
        for (name, file, method) in pos {
            println!("{s}Status: adding sample {b}\"{x}\"{s} to the sample bank.",
                    s = UC::Std, b = UC::Blue, x = name);
            if let Err(msg) = self.sb.add(name.clone(), &file, SampleLoadMethod::from(&method)){
                println!("{}{}", UC::Red, msg);
                to_exclude.push(name);
            }
        }
        do_excluding!(to_exclude, new_samples, self.cur_samples);

        // Same for resources
        let (pos, neg) = diff(&self.cur_resources, &new_resources);
        for (name, _) in neg {
            println!("{s}Info: resource {b}\"{x}\"{s} will be removed.",
                    s = UC::Std, b = UC::Blue, x = name);
            self.bb.mark_dead(&name);
        }
        println!("{}Status: refreshing resources.", UC::Std);
        self.bb.refresh();
        let mut to_exclude = Vec::new();
        for (name, file) in pos{
            if let Err(msg) = self.bb.add(name.clone(), &file){
                println!("{}{}", UC::Red, msg);
                to_exclude.push(name);
            }
        }
        do_excluding!(to_exclude, new_resources, self.cur_resources);

        // Just reload all midi, so you can easily import newly inplace generated files
        self.fb.reset();
        for (name, file) in midis{
            if let Err(msg) = self.fb.add_floww(name, &file){
                println!("{}{}", UC::Red, msg);
                return;
            }
        }
        for name in streams{
            println!("{}", name);
            self.fb.declare_stream(name);
        }

        // Also don't recreate plugins
        // TODO: make renaming possible
        #[cfg(feature = "lv2")]
        {
            let (pos, neg) = diff(&self.cur_lv2plugins, &new_lv2plugins);
            for (name, _) in neg { // TODO: make plugins removable
                self.host.remove_plugin(&name);
            }
            let mut to_exclude = Vec::new();
            for (name, uri) in pos {
                if let Err(e) = self.host.add_plugin(&uri, name.clone()){
                    println!("{r}Couldn't load Lv2 plugin with name: {b}\"{n}\"{r} and uri: {b}\"{u}\"{r}.",
                        r = UC::Red, b = UC::Blue, n = name, u = uri);
                    match e{
                        AddPluginError::CapacityReached => {
                            println!("{}\tCapacity reached!", UC::Red);
                        },
                        AddPluginError::WorldIsNull => {
                            println!("{}\tWorld is null!", UC::Red);
                        },
                        AddPluginError::PluginIsNull => {
                            println!("{}\tPlugin is null!", UC::Red);
                        },
                        AddPluginError::MoreThanTwoInOrOutAudioPorts(ins, outs) => {
                            println!("{}\tPlugin has more than two input or output audio ports!", UC::Red);
                            println!("{r}\tAudio inputs: {b}{i}{r}, audio outputs: {b}{o}",
                                r = UC::Red, b = UC::Blue, i = ins, o = outs);
                        },
                        AddPluginError::MoreThanOneAtomPort(atomports) => {
                            println!("{r}\tPlugin has more than one atom ports! Atom ports: {b}{a}{r}.",
                                r = UC::Red, b = UC::Blue, a = atomports);
                        },
                        AddPluginError::PortNeitherInputOrOutput => {
                            println!("{}\tPlugin has a port that is neither input or output.", UC::Red);
                        },
                        AddPluginError::PortNeitherControlOrAudioOrOptional => {
                            println!("{}\tPlugin has a port that is neither control, audio or optional.", UC::Red);
                        },
                    }
                    to_exclude.push(name.clone());
                }
                println!("{s}Info: added plugin {b}{n}{s} with uri {b}{u}{s}.",
                    s = UC::Std, b = UC::Blue, n = name, u = uri);
            }
            do_excluding!(to_exclude, new_lv2plugins, self.cur_lv2plugins);

            // need diff to see what params we need to reset
            let (pos, neg) = diff(&self.cur_lv2params, &new_lv2params);
            for (plugin, name, _) in neg { // TODO: make params resetable in Lv2hm
                self.host.reset_value(&plugin, &name);
            }
            for (plugin, name, value) in pos{
                self.host.set_value(&plugin, &name, value);
            }

            self.cur_lv2params = new_lv2params;
        }

        // just rebuild the damn thing, if it becomes problematic i'll do something about it,
        // probably :)
        println!("{}Status: rebuilding graph.", UC::Std);
        self.g.reset();
        macro_rules! get_index{
            ($obj:expr, $arg:expr, $name:expr, $category:expr) => {
                match $obj.get_index($arg){
                    Some(i) => i,
                    None => {
                        println!("{}Could not get {} index for vertex {}\"{}\"{}.",
                            UC::Red, $category, UC::Blue, $name, UC::Std);
                        return;
                    }
                }
            }
        }
        for (name, gain, angle) in &sums { self.g.add(Vertex::new(bl, *gain, *angle, 0.0, VertexExt::sum()), name.to_owned()); }
        for (name, gain, angle) in &norms { self.g.add(Vertex::new(bl, *gain, *angle, 0.0, VertexExt::normalize()), name.to_owned()); }
        for (name, gain, angle, sample) in &sampleloops {
            let index = get_index!(self.sb, sample, name, "sample");
            self.g.add(Vertex::new(bl, *gain, *angle, 0.0, VertexExt::sample_loop(index)), name.to_owned());
        }
        for (name, gain, angle, sample, floww, note) in &samplemultis {
            let sample = get_index!(self.sb, sample, name, "sample");
            let floww = get_index!(self.fb, floww, name, "floww");
            let note = if note < &0 { None }
            else { Some(*note as usize) };
            self.g.add(Vertex::new(bl, *gain, *angle, 0.0, VertexExt::sample_multi(sample, floww, note)), name.to_owned());
        }
        for (name, gain, angle, sample, floww, note, lerp_len) in &samplelerps {
            let sample = get_index!(self.sb, sample, name, "sample");
            let floww = get_index!(self.fb, floww, name, "floww");
            let note = if note < &0 { None }
            else { Some(*note as usize) };
            let lerp_len = (*lerp_len).max(0) as usize;
            self.g.add(Vertex::new(bl, *gain, *angle, 0.0, VertexExt::sample_lerp(sample, floww, note, lerp_len)), name.to_owned());
        }
        for (name, gain, angle, floww) in &debugsines {
            let floww = get_index!(self.fb, floww, name, "floww");
            self.g.add(Vertex::new(bl, *gain, *angle, 0.0, VertexExt::debug_sine(floww)), name.to_owned());
        }
        for (name, gain, angle, floww, sq_vel, sq_z, sq_arr, tf_vel, tf_z, tf_arr, tr_vel, tr_arr) in &synths {
            let floww = get_index!(self.fb, floww, name, "floww");
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
        for (name, gain, angle, floww, adsr_conf, resource) in &sampsyns {
            let floww = get_index!(self.fb, floww, name, "floww");

            let adsr = if let Some(config) = build_adsr_conf(adsr_conf){ config }
            else { panic!("ADSR config must have 6 or 9 elements"); };

            let buf_ind = if let Some(i) = self.bb.get_index(resource){ i }
            else { panic!("Could not find resource named {}!", resource); };

            let table = if let Some(t) = parse_wavetable_from_buffer(self.bb.get_buffer(buf_ind)) { t }
            else {
                println!("{s}Could not parse wavetable from resource {b}\"{r}\"{s}, using default table!",
                    s = UC::Std, b = UC::Blue, r = resource);
                WaveTable::default()
            };

            self.g.add(Vertex::new(bl, *gain, *angle, 0.0,
                VertexExt::sampsyn(floww, adsr, table)), name.to_owned());
        }
        #[cfg(feature = "lv2")]
        {
            for (name, gain, angle, wet, plugin) in &lv2fxs {
                let index = get_index!(self.host, plugin, name, "plugin");
                self.g.add(Vertex::new(bl, *gain, *angle, *wet, VertexExt::lv2fx(index)), name.to_owned());
            }
        }
        for (name, gain, angle, wet, floww, use_off, use_max, note, conf_arr) in &adsrs {
            let floww = get_index!(self.fb, floww, name, "floww");
            let note = if note < &0 { None }
            else { Some(*note as usize) };
            let conf = if let Some(config) = build_adsr_conf(conf_arr){
                config
            } else {
                panic!("ADSR config must have 6 or 9 elements");
            };
            self.g.add(Vertex::new(bl, *gain, *angle, *wet, VertexExt::adsr(*use_off, *use_max, conf, note, floww)), name.to_owned());
        }
        for (name, gain, angle, wet, cut_off_hz_low, cut_off_hz_high, pass) in &bandpasses {
            self.g.add(Vertex::new(bl, *gain, *angle, *wet,
                    VertexExt::band_pass(*cut_off_hz_low, *cut_off_hz_high, *pass, psr)),
                name.to_owned());
        }

        for (a, b) in &edges { self.g.connect(a, b); }

        self.g.set_output(&self.output_vertex);
        if !self.g.check_graph(){
            println!("{}TermDaw: graph check failed!", UC::Red);
            return;
        }

        self.g.reset_normalize_vertices();

        println!("{}Ok: refreshed.", UC::Green);
        self.loaded = true;
    }

    pub fn scan_exact(&mut self){
        self.g.true_normalize_scan(&self.sb, &mut self.fb, &mut self.host, self.cs);
    }

    pub fn render(&mut self) {
        println!("{}Status: started rendering", UC::Std);
        let psr = self.config.settings.project_samplerate();
        let bl = self.config.settings.buffer_length();

        let (msr, mbd) = self.sb.get_max_sr_bd();
        if psr > self.render_sr{
            println!("{y}TermDaw: warning: render will down sample from {b}{p}{y}(project s.r.) to {b}{r}{y}.",
                y = UC::Yellow, b = UC::Blue, p = psr, r = self.render_sr);
        }
        if msr > self.render_sr{
            println!("{y}TermDaw: warning: render will down sample from peak input quality({b}{m}{y}) to {b}{r}{y}.",
                y = UC::Yellow, b = UC::Blue, m = msr, r = self.render_sr);
        }
        if !(self.bd == 8 || self.bd == 16 || self.bd == 24 || self.bd == 32) {
            println!("{r}Bitdepth of {b}{bd}{r} not supported: choose bitdepth in {{8, 16, 24, 32}}.",
                r = UC::Red, b = UC::Blue, bd = self.bd);
            return;
        }
        if mbd > self.bd{
            println!("{y}TermDaw: warning: render will lose bitdepth from peak input quality({b}{m}{y} bits) to {b}{bd}{y} bits",
                y = UC::Yellow, b = UC::Blue,  m = mbd, bd = self.bd);
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
        println!("{}Ok: done rendering.", UC::Green);
    }
}


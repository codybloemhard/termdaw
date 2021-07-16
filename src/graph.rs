use crate::sample::{ Sample, SampleBank };
use crate::floww::{ FlowwBank };
use crate::adsr::*;

use lv2hm::Lv2Host;

use std::collections::{ HashMap, VecDeque };
use core::f32::consts::PI;

pub struct Graph{
    vertices: Vec<Vertex>,
    edges: Vec<Vec<usize>>,
    names: Vec<String>,
    name_map: HashMap<String, usize>,
    ran_status: Vec<bool>,
    output_vertex: Option<usize>,
    max_buffer_len: usize,
    sr: usize,
    t: usize,
}

impl Graph{
    pub fn new(max_buffer_len: usize, sr: usize) -> Self{
        Self{
            vertices: Vec::new(),
            edges: Vec::new(),
            name_map: HashMap::new(),
            names: Vec::new(),
            ran_status: Vec::new(),
            output_vertex: None,
            max_buffer_len,
            sr,
            t: 0,
        }
    }

    pub fn reset(&mut self){
        self.vertices.clear();
        self.edges.clear();
        self.name_map.clear();
        self.names.clear();
        self.ran_status.clear();
        self.output_vertex = None;
        self.t = 0;
    }

    pub fn add(&mut self, node: Vertex, name: String){
        self.vertices.push(node);
        self.ran_status.push(false);
        self.edges.push(Vec::new());
        let n = self.vertices.len() - 1;
        self.name_map.insert(name.clone(), n);
        self.names.push(name);
    }

    fn connect_internal(&mut self, a: usize, b: usize) -> bool{
        // basic checks
        if a == b { return false; }
        let len = self.vertices.len();
        if a >= len { return false; }
        if b >= len { return false; }
        if !self.vertices[b].has_input() { return false; }
        // loop detection:
        fn has_loop(x: usize, b: usize, edges: &[Vec<usize>]) -> bool{
            if x == b { return true; }
            for y in &edges[x]{
                if has_loop(*y, b, edges) { return true; };
            }
            false
        }
        if has_loop(a, b, &self.edges) { return false; }
        // connect a to b: a -> b, a into b
        // reverse: for such b we want to know which a's we should query
        self.edges[b].push(a);
        true
    }

    pub fn connect(&mut self, a: &str, b: &str) -> bool{
        let a_res = self.name_map.get(a);
        let b_res = self.name_map.get(b);
        if a_res.is_none() {
            println!("TermDaw: warning: vertex \"{}\" cannot be found and thus can't be connected.", a);
            return false;
        }
        if b_res.is_none() {
            println!("TermDaw: warning: vertex \"{}\" cannot be found and thus can't be connected to.", b);
            return false;
        }
        let a_index = *a_res.unwrap();
        let b_index = *b_res.unwrap();
        self.connect_internal(a_index, b_index)
    }

    fn run_vertex(&mut self, t: usize, sb: &SampleBank, fb: &mut FlowwBank, host: &mut Lv2Host, index: usize, is_scan: bool){
        if index >= self.vertices.len() { return; }
        if self.ran_status[index] { return; }
        self.ran_status[index] = true;
        let edges = self.edges[index].clone();
        for incoming in &edges{
            self.run_vertex(t, sb, fb, host, *incoming, is_scan);
        }
        // Vertex buffers exist as long at the graph exists: we never delete vertices
        // Safe: we mutate vertex A (&mut A) and read dat from incoming vertices [B] (&[B])
        // TODO: maybe use arena? https://crates.io/crates/typed-arena
        unsafe {
            let ins = edges.iter().map(|incoming|{
                &*(self.vertices[*incoming].read_buffer() as *const _)
            }).collect::<Vec<_>>();
            self.vertices[index].generate(t, self.sr, sb, fb, host, self.max_buffer_len, is_scan, ins);
        }
    }

    pub fn set_time(&mut self, time: usize){
        self.t = time;
        for v in &mut self.vertices{
            v.set_time(time);
        }
    }

    pub fn change_time(&mut self, delta: usize, plus: bool) -> usize{
        let new_time = if plus { self.t + delta }
        else { self.t - delta.min(self.t) };
        self.set_time(new_time);
        new_time
    }

    pub fn get_time(&self) -> usize{
        self.t
    }

    pub fn set_output(&mut self, vert: &str) -> bool{
        if let Some(index) = self.name_map.get(vert){
            self.output_vertex = Some(*index);
            true
        } else {
            false
        }
    }

    pub fn check_graph(&self) -> bool{
        let output = if let Some(out) = self.output_vertex{ out }
        else {
            println!("TermDaw: error: output vertex not found.");
            return false;
        };
        if self.edges[output].is_empty(){
            println!("TermDaw: error: output receives no inputs.");
            return false;
        }
        let mut set = vec![false; self.vertices.len()];
        fn find_connected_component(x: usize, edges: &[Vec<usize>], set: &mut Vec<bool>){
            set[x] = true;
            for y in &edges[x]{
                find_connected_component(*y, edges, set);
            }
        }
        find_connected_component(output, &self.edges, &mut set);
        for (i, x) in set.into_iter().enumerate(){
            if x { continue; }
            println!("TermDaw: warning: vertex \"{}\" does not reach output.", self.names[i]);
        }
        true
    }

    fn reset_ran_stati(&mut self){
        for ran in &mut self.ran_status{
            *ran = false;
        }
    }

    pub fn render(&mut self, sb: &SampleBank, fb: &mut FlowwBank, host: &mut Lv2Host) -> Option<&Sample>{
        self.reset_ran_stati();
        if let Some(index) = self.output_vertex{
            self.run_vertex(self.t, sb, fb, host, index, false);
            self.t += self.max_buffer_len;
            Some(self.vertices[index].read_buffer())
        } else {
            None
        }
    }

    pub fn scan(&mut self, sb: &SampleBank, fb: &mut FlowwBank, host: &mut Lv2Host, chunks: usize){
        let i = if let Some(index) = self.output_vertex{ index }
        else { return; };
        fb.set_time(0);
        for j in 0..chunks {
            self.reset_ran_stati();
            self.run_vertex(j * self.max_buffer_len, sb, fb, host, i, true);
            fb.set_time_to_next_block();
        }
        self.set_time(0);
        fb.set_time(0);
    }
}

pub struct Vertex{
    buf: Sample,
    gain: f32,
    angle: f32,
    ext: VertexExt,
}

impl Vertex{
    pub fn new(bl: usize, gain: f32, angle: f32, ext: VertexExt) -> Self{
        Self{
            buf: Sample::new(bl),
            gain,
            angle: angle.min(90.0).max(-90.0),
            ext,
        }
    }

    fn read_buffer(&self) -> &Sample{
        &self.buf
    }

    fn generate(&mut self, t: usize, sr: usize, sb: &SampleBank, fb: &mut FlowwBank, host: &mut Lv2Host, len: usize, is_scan: bool, res: Vec<&Sample>){
        let len = self.buf.len().min(len);
        self.ext.generate(t, sr, sb, fb, host, self.gain, self.angle, &mut self.buf, len, res, is_scan);
    }

    // Whether or not you can connect another vertex to (into) this one
    fn has_input(&self) -> bool{
        self.ext.has_input()
    }

    pub fn set_time(&mut self, t: usize){
        self.ext.set_time(t);
    }

    pub fn set_gain(&mut self, gain: f32){
        self.gain = gain;
    }

    pub fn set_angle(&mut self, angle: f32){
        self.angle = angle.min(90.0).max(-90.0);
    }
}

pub enum VertexExt{
    Sum,
    Normalize{
        max: f32,
    },
    SampleLoop{
        sample_index: usize,
        playing: bool,
        t: usize,
    },
    SampleFlowwMulti{
        sample_index: usize,
        floww_index: usize,
        note: Option<usize>,
        playing: bool,
        ts: VecDeque<(i64, f32)>,
    },
    SampleFlowwLerp{
        sample_index: usize,
        floww_index: usize,
        note: Option<usize>,
        lerp_len: usize,
        countdown: usize,
        primary: (i64, f32),
        ghost: (i64, f32),
        playing: bool,
    },
    SineFloww{
        floww_index: usize,
        notes: Vec<(f32, f32)>,
    },
    Lv2fx{
        index: usize,
    },
    Adsr{
        use_off: bool,
        conf: AdsrConf,
        floww_index: usize,
        note: Option<usize>,
        primary: (f32, f32, bool),
        ghost: (f32, f32, bool),
    },
}

impl VertexExt{
    pub fn sum() -> Self{
        Self::Sum
    }

    pub fn normalize() -> Self{
        Self::Normalize{
            max: 0.0, // value on scan
        }
    }

    pub fn sample_loop(sample_index: usize) -> Self{
        Self::SampleLoop{
            sample_index,
            playing: true,
            t: 0,
        }
    }

    pub fn sample_floww_multi(sample_index: usize, floww_index: usize, note: Option<usize>) -> Self{
        Self::SampleFlowwMulti{
            sample_index,
            floww_index,
            ts: VecDeque::new(),
            note,
            playing: true,
        }
    }

    pub fn sample_floww_lerp(sample_index: usize, floww_index: usize, note: Option<usize>, lerp_len: usize) -> Self{
        Self::SampleFlowwLerp{
            sample_index,
            floww_index,
            note,
            lerp_len,
            countdown: 0,
            primary: (0, 0.0),
            ghost: (0, 0.0),
            playing: true,
        }
    }

    pub fn sine_floww(floww_index: usize) -> Self{
        Self::SineFloww{
            floww_index,
            notes: Vec::new(),
        }
    }

    pub fn lv2fx(plugin_index: usize) -> Self{
        Self::Lv2fx{
            index: plugin_index,
        }
    }

    pub fn adsr(use_off: bool, conf: AdsrConf, note: Option<usize>, floww_index: usize) -> Self{
        Self::Adsr{
            use_off,
            conf,
            note,
            floww_index,
            primary: (0.0, 0.0, true),
            ghost: (0.0, 0.0, true),
        }
    }

    fn set_time(&mut self, time: usize){
        if let Self::SampleLoop{ t, .. } = self { *t = time; }
    }

    fn generate(&mut self, t: usize, sr: usize, sb: &SampleBank, fb: &mut FlowwBank, host: &mut Lv2Host, gain: f32, angle: f32, buf: &mut Sample, len: usize, res: Vec<&Sample>, is_scan: bool){
        match self{
            Self::Sum => {
                sum_gen(buf, len, res);
            },
            Self::Normalize { max } => {
                normalize_gen(buf, len, res, max, is_scan);
            },
            Self::SampleLoop { playing, t, sample_index } => {
                sample_loop_gen(buf, sb, len, playing, t, *sample_index);
            },
            Self::SampleFlowwMulti { playing, ts, sample_index, floww_index, note } => {
                sample_floww_multi_gen(buf, sb, fb, len, playing, ts, *sample_index, *floww_index, *note);
            },
            Self::SampleFlowwLerp { playing, sample_index, floww_index, note, countdown, lerp_len, primary, ghost } => {
                sample_floww_lerp_gen(buf, sb, fb, len, playing, *sample_index, *floww_index, *note, *lerp_len, countdown, primary, ghost);
            },
            Self::SineFloww { floww_index, notes } => {
                sine_floww_gen(buf, fb, len, *floww_index, notes, t, sr);
            },
            Self::Lv2fx { index } => {
                lv2fx_gen(buf, len, res, *index, host);
            },
            Self::Adsr { use_off, conf, note, floww_index, primary, ghost } => {
                adsr_gen(buf, len, res, fb, *use_off, *floww_index, sr, &conf, *note, primary, ghost);
            }
        }
        buf.apply_angle(angle, len);
        buf.apply_gain(gain, len);
    }

    fn has_input(&self) -> bool{
        match self{
            Self::Sum => true,
            Self::Normalize { .. } => true,
            Self::SampleLoop { .. } => false,
            Self::SampleFlowwMulti { .. } => false,
            Self::SampleFlowwLerp { .. } => false,
            Self::SineFloww { .. } => false,
            Self::Lv2fx { .. } => true,
            Self::Adsr { .. } => true,
         }
    }
}

fn sum_inputs(buf: &mut Sample, len: usize, res: Vec<&Sample>){
    buf.zero();
    for r in res{
        let l = r.len().min(len);
        for i in 0..l{
            buf.l[i] += r.l[i];
            buf.r[i] += r.r[i];
        }
    }
}

fn sum_gen(buf: &mut Sample, len: usize, res: Vec<&Sample>){
    sum_inputs(buf, len, res);
}

fn normalize_gen(buf: &mut Sample, len: usize, res: Vec<&Sample>, max: &mut f32, is_scan: bool){
    sum_inputs(buf, len, res);
    if is_scan{
        *max = buf.scan_max(len).max(*max);
    } else {
        buf.scale(len, 1.0 / *max);
    }
}

fn sample_loop_gen(buf: &mut Sample, sb: &SampleBank, len: usize, playing: &mut bool, t: &mut usize, sample_index: usize){
    let sample = sb.get_sample(sample_index);
    if *playing{
        let l = sample.len();
        for i in 0..len{
            buf.l[i] = sample.l[(*t + i) % l];
            buf.r[i] = sample.r[(*t + i) % l];
        }
        *t += len;
    } else {
        buf.zero();
    }
}

fn sample_floww_multi_gen(buf: &mut Sample, sb: &SampleBank, fb: &mut FlowwBank, len: usize, playing: &mut bool, ts: &mut VecDeque<(i64, f32)>, sample_index: usize, floww_index: usize, target_note: Option<usize>){
    if *playing{
        let sample = sb.get_sample(sample_index);
        fb.start_block(floww_index);
        for i in 0..len{
            if let Some((note, v)) = fb.get_block_drum(floww_index, i){
                let ok = if let Some(n) = target_note{
                    (note - n as f32).abs() < 0.01
                }
                else { true };
                if ok{
                    ts.push_back((-(i as i64), v)); // line up with i so that (t + i) = (-i + i) = 0 is the first frame copied
                }
            }
            buf.l[i] = 0.0;
            buf.r[i] = 0.0;
            let mut pops = 0;
            for (t, vel) in ts.iter(){
                let pos = (*t + i as i64).max(0) as usize;
                if pos >= sample.len() {
                    pops += 1;
                } else {
                    buf.l[i] += sample.l[pos] * *vel;
                    buf.r[i] += sample.r[pos] * *vel;
                }
            }
            for _ in 0..pops{
                ts.pop_front();
            }
        }
        for (t, _) in ts{
            *t += len as i64;
        }
    } else {
        buf.zero();
    }
}

fn sample_floww_lerp_gen(buf: &mut Sample, sb: &SampleBank, fb: &mut FlowwBank, len: usize, playing: &mut bool, sample_index: usize,
    floww_index: usize, target_note: Option<usize>, lerp_len: usize, countdown: &mut usize, primary: &mut (i64, f32), ghost: &mut (i64, f32)){
    if *playing{
        let sample = sb.get_sample(sample_index);
        fb.start_block(floww_index);
        for i in 0..len{
            if let Some((note, v)) = fb.get_block_drum(floww_index, i){
                let ok = if let Some(n) = target_note{
                    (note - n as f32).abs() < 0.01
                }
                else { true };
                if ok{
                    *ghost = *primary;
                    *primary = (-(i as i64), v); // line up with i so that (t + i) = (-i + i) = 0 is the first frame copied
                    *countdown = lerp_len;
                }
            }
            let primary_pos = ((primary.0 + i as i64).max(0) as usize).min(sample.len() - 1);
            let mut l = sample.l[primary_pos] * primary.1;
            let mut r = sample.r[primary_pos] * primary.1;
            if *countdown > 0{
                *countdown -= 1;
                let t = *countdown as f32 / lerp_len as f32;
                let ghost_pos = ((ghost.0 + i as i64).max(0) as usize).min(sample.len() - 1);
                let gl = sample.l[ghost_pos] * ghost.1;
                let gr = sample.r[ghost_pos] * ghost.1;
                l = gl * t + l * (1.0 - t);
                r = gr * t + r * (1.0 - t);
            }
            buf.l[i] = l;
            buf.r[i] = r;
        }
        primary.0 += len as i64;
        ghost.0 += len as i64;
    } else {
        buf.zero();
    }
}

fn sine_floww_gen(buf: &mut Sample, fb: &mut FlowwBank, len: usize, floww_index: usize, notes: &mut Vec<(f32, f32)>, t: usize, sr: usize){
    fb.start_block(floww_index);
    for i in 0..len{
        for (on, note, vel) in fb.get_block_simple(floww_index, i){
            if on{
                let mut has = false;
                for (n, v) in notes.iter_mut(){
                    if (*n - note).abs() < 0.001{
                        *v = vel;
                        has = true;
                        break;
                    }
                }
                if !has {
                    notes.push((note, vel));
                }
            } else {
                notes.retain(|x| (x.0 - note).abs() > 0.001);
            }
        }

        buf.l[i] = 0.0;
        buf.r[i] = 0.0;
        for (note, vel) in notes.iter(){
            let time = (t + i) as f32 / sr as f32;
            let hz = 440.0 * (2.0f32).powf((note - 69.0) / 12.0);
            let s = (time * hz * 2.0 * PI).sin() * vel;
            buf.l[i] += s;
            buf.r[i] += s;
        }
    }
}

fn lv2fx_gen(buf: &mut Sample, len: usize, res: Vec<&Sample>, index: usize, host: &mut Lv2Host){
    sum_inputs(buf, len, res);
    if let Some(outp_ref) = host.apply_plugin_n_frames(index, &buf.clone().deinterleave()){
        for i in 0..len{
            buf.l[i] = outp_ref[i * 2];
            buf.r[i] = outp_ref[i * 2 + 1];
        }
    }
}

fn adsr_gen(buf: &mut Sample, len: usize, res: Vec<&Sample>, fb: &mut FlowwBank, use_off: bool, floww_index: usize, sr: usize,
            conf: &AdsrConf, note: Option<usize>, primary: &mut (f32, f32, bool), ghost: &mut (f32, f32, bool)){
    sum_inputs(buf, len, res);
    fb.start_block(floww_index);
    if use_off{
        for i in 0..len{
            for (on, n, v) in fb.get_block_simple(floww_index, i){
                if let Some(target) = note{
                    if (target as f32 - n).abs() > 0.01 { continue; }
                }
                if on{
                    *ghost = *primary;
                    *primary = (0.0, v, true);
                } else if ghost.2 {
                    ghost.2 = false;
                } else {
                    primary.2 = false;
                }
            }
            let pvel = if primary.2{ apply_ads(conf, primary.0) * primary.1 }
            else { apply_r(conf, primary.0) * primary.1 };
            let gvel = if ghost.2{ apply_ads(conf, ghost.0) * ghost.1 }
            else { apply_r(conf, ghost.0) * ghost.1 };
            let vel = pvel.max(gvel);

            buf.l[i] *= vel;
            buf.r[i] *= vel;
        }
    } else {
        for i in 0..len{
            if let Some((n, v)) = fb.get_block_drum(floww_index, i){
                if let Some(target) = note{
                    if (target as f32 - n).abs() > 0.01 { continue; }
                }
                *ghost = *primary;
                *primary = (0.0, v, true);
            }
            let offset = i as f32 / sr as f32;
            let pvel = apply_adsr(conf, primary.0 + offset);
            let gvel = apply_adsr(conf, ghost.0 + offset);
            let vel = pvel.max(gvel);

            buf.l[i] *= vel;
            buf.r[i] *= vel;
        }
    }
    primary.0 += len as f32 / sr as f32;
    ghost.0 += len as f32 / sr as f32;
}

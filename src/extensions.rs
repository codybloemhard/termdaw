use crate::{
    sample::{ Sample, SampleBank },
    floww::FlowwBank,
    adsr::*,
    synth::*,
    graph::GenArgs,
    lv2::Lv2Host,
};

use core::f32::consts::PI;
use std::collections::VecDeque;

use sampsyn::*;

pub enum VertexExt{
    Sum,
    Normalize{
        max: f32,
        scan_max: f32,
    },
    SampleLoop{
        sample_index: usize,
        t: usize,
    },
    SampleMulti{
        sample_index: usize,
        floww_index: usize,
        note: Option<usize>,
        ts: VecDeque<(i64, f32)>,
    },
    SampleLerp{
        sample_index: usize,
        floww_index: usize,
        note: Option<usize>,
        lerp_len: usize,
        countdown: usize,
        primary: (i64, f32),
        ghost: (i64, f32),
    },
    DebugSine{
        floww_index: usize,
        notes: Vec<(f32, f32)>,
    },
    Synth{
        floww_index: usize,
        square_conf: OscConf,
        topflat_conf: OscConf,
        triangle_conf: OscConf,
        notes: Vec<(f32, f32, f32, f32)>,
    },
    SampSyn{
        floww_index: usize,
        adsr: AdsrConf,
        wave_table: WaveTable,
        notes: Vec<(f32, f32, f32, f32, WaveTableState)>,
    },
    #[cfg(feature = "lv2")]
    Lv2fx{
        index: usize,
    },
    Adsr{
        use_off: bool,
        use_max: bool,
        conf: AdsrConf,
        floww_index: usize,
        note: Option<usize>,
        primary: (f32, f32, f32),
        ghost: (f32, f32, f32),
    },
    BandPass{
        lgamma: f32,
        hgamma: f32,
        lprevl: f32,
        lprevr: f32,
        hprevl: f32,
        hprevr: f32,
        first: bool,
        pass: bool,
    }
}

impl VertexExt{
    pub fn sum() -> Self{
        Self::Sum
    }

    pub fn normalize() -> Self{
        Self::Normalize{
            max: 0.0, // values on scan
            scan_max: 0.0,
        }
    }

    pub fn sample_loop(sample_index: usize) -> Self{
        Self::SampleLoop{
            sample_index,
            t: 0,
        }
    }

    pub fn sample_multi(sample_index: usize, floww_index: usize, note: Option<usize>) -> Self{
        Self::SampleMulti{
            sample_index,
            floww_index,
            ts: VecDeque::new(),
            note,
        }
    }

    pub fn sample_lerp(
        sample_index: usize, floww_index: usize, note: Option<usize>, lerp_len: usize
    ) -> Self{
        Self::SampleLerp{
            sample_index,
            floww_index,
            note,
            lerp_len,
            countdown: 0,
            primary: (0, 0.0),
            ghost: (0, 0.0),
        }
    }

    pub fn debug_sine(floww_index: usize) -> Self{
        Self::DebugSine{
            floww_index,
            notes: Vec::new(),
        }
    }

    pub fn synth(
        floww_index: usize, square_conf: OscConf, topflat_conf: OscConf, triangle_conf: OscConf
    ) -> Self{
        Self::Synth{
            floww_index,
            notes: Vec::new(),
            square_conf,
            topflat_conf,
            triangle_conf,
        }
    }

    pub fn sampsyn(floww_index: usize, adsr: AdsrConf, wave_table: WaveTable) -> Self{
        Self::SampSyn{
            floww_index,
            adsr,
            wave_table,
            notes: Vec::new(),
        }
    }

    #[cfg(feature = "lv2")]
    pub fn lv2fx(plugin_index: usize) -> Self{
        Self::Lv2fx{
            index: plugin_index,
        }
    }

    pub fn adsr(
        use_off: bool, use_max: bool, conf: AdsrConf, note: Option<usize>, floww_index: usize
    ) -> Self{
        Self::Adsr{
            use_off,
            use_max,
            conf,
            note,
            floww_index,
            primary: (0.0, 0.0, 0.0),
            ghost: (0.0, 0.0, 0.0),
        }
    }

    pub fn band_pass(
        cut_off_hz_low: f32, cut_off_hz_hig: f32, pass: bool, sampling_hz: usize
    ) -> Self{
        let lco = cut_off_hz_low.min(20000.0).max(0.0);
        let hco = cut_off_hz_hig.min(20000.0).max(0.0);
        let lgamma = 1.0 - std::f32::consts::E.powf(
            -2.0 * std::f32::consts::PI * lco / sampling_hz as f32
        );
        let hgamma = 1.0 - std::f32::consts::E.powf(
            -2.0 * std::f32::consts::PI * hco / sampling_hz as f32
        );
        Self::BandPass{
            lgamma,
            hgamma,
            lprevl: 0.0,
            lprevr: 0.0,
            hprevl: 0.0,
            hprevr: 0.0,
            first: true,
            pass,
        }
    }

    pub fn set_time(&mut self, time: usize){
        match self{
            Self::SampleLoop { t, .. } => { *t = time; },
            Self::DebugSine { notes, .. } => { notes.clear(); },
            Self::Synth { notes, .. } => { notes.clear(); },
            Self::BandPass { first, .. } => { *first = true; },
            _ => {  },
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn generate(
        &mut self, ga: GenArgs, sb: &SampleBank, fb: &mut FlowwBank, _host: &mut Lv2Host,
        gain: f32, angle: f32, wet: f32, buf: &mut Sample, res: Vec<&Sample>
    ){
        let (t, sr, len, is_scan) = ga;
        if self.has_input(){
            sum_inputs(buf, len, res);
        }
        match self{
            Self::Sum => { },
            Self::Normalize { max, scan_max } => {
                normalize_gen(buf, len, max, scan_max, is_scan);
            },
            Self::SampleLoop { t, sample_index } => {
                sample_loop_gen(buf, sb, len, t, *sample_index);
            },
            Self::SampleMulti { ts, sample_index, floww_index, note } => {
                sample_multi_gen(buf, sb, fb, len, ts, *sample_index, *floww_index, *note);
            },
            Self::SampleLerp {
                sample_index, floww_index, note, countdown, lerp_len, primary, ghost
            } => {
                sample_lerp_gen(
                    buf, sb, fb, len, *sample_index, *floww_index, *note, *lerp_len,
                    countdown, primary, ghost
                );
            },
            Self::DebugSine { floww_index, notes } => {
                debug_sine_gen(buf, fb, len, *floww_index, notes, t, sr);
            },
            Self::Synth { floww_index, notes, square_conf, topflat_conf, triangle_conf } => {
                synth_gen(
                    buf, fb, len, *floww_index, notes, square_conf, topflat_conf, triangle_conf,
                    t, sr
                );
            },
            Self::SampSyn { floww_index, notes, adsr, wave_table } => {
                sampsyn_gen(buf, fb, len, *floww_index, notes, adsr, wave_table, sr);
            },
            #[cfg(feature = "lv2")]
            Self::Lv2fx { index } => {
                lv2fx_gen(buf, len, wet, *index, _host);
            },
            Self::Adsr { use_off, use_max, conf, note, floww_index, primary, ghost } => {
                adsr_gen(
                    buf, len, fb, wet, *use_off, *use_max, *floww_index, sr, conf, *note,
                    primary, ghost
                );
            },
            Self::BandPass { lprevl, lprevr, hprevl, hprevr, lgamma, hgamma, first, pass } => {
                band_pass_gen(
                    buf, len, wet, first, *pass, *lgamma, *hgamma, lprevl, lprevr, hprevl, hprevr
                );
            }
        }
        buf.apply_angle(angle, len);
        buf.apply_gain(gain, len);
    }

    pub fn has_input(&self) -> bool{
        match self{
            Self::Sum => true,
            Self::Normalize { .. } => true,
            Self::SampleLoop { .. } => false,
            Self::SampleMulti { .. } => false,
            Self::SampleLerp { .. } => false,
            Self::DebugSine { .. } => false,
            Self::Synth { .. } => false,
            Self::SampSyn { .. } => false,
            #[cfg(feature = "lv2")]
            Self::Lv2fx { .. } => true,
            Self::Adsr { .. } => true,
            Self::BandPass { .. } => true,
        }
    }

    pub fn reset_scan_normalization(&mut self){
        if let Self::Normalize { scan_max, .. } = self{
            *scan_max = 0.0;
        }
    }

    pub fn apply_scan_normalization(&mut self){
        if let Self::Normalize { scan_max, max } = self{
            *max = *scan_max;
        }
    }

    pub fn reset_normalization(&mut self){
        if let Self::Normalize{ max, .. } = self{
            *max = 0.000001;
        }
    }

    pub fn get_normalization_value(&self) -> f32{
        if let Self::Normalize{ max, .. } = self{
            *max
        } else {
            -1.0
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

fn normalize_gen(buf: &mut Sample, len: usize, max: &mut f32, scan_max: &mut f32, is_scan: bool){
    let buf_max = buf.scan_max(len);
    if is_scan{
        *scan_max = buf_max.max(*scan_max);
    } else {
        *max = buf_max.max(*max);
    }
    buf.scale(len, 1.0 / *max);
}

fn sample_loop_gen(
    buf: &mut Sample, sb: &SampleBank, len: usize, t: &mut usize, sample_index: usize
){
    let sample = sb.get_sample(sample_index);
    let l = sample.len();
    for i in 0..len{
        buf.l[i] = sample.l[(*t + i) % l];
        buf.r[i] = sample.r[(*t + i) % l];
    }
    *t += len;
}

#[allow(clippy::too_many_arguments)]
fn sample_multi_gen(
    buf: &mut Sample, sb: &SampleBank, fb: &mut FlowwBank, len: usize,
    ts: &mut VecDeque<(i64, f32)>, sample_index: usize, floww_index: usize,
    target_note: Option<usize>
){
    let sample = sb.get_sample(sample_index);
    fb.start_block(floww_index);
    for i in 0..len{
        if let Some((note, v)) = fb.get_block_drum(floww_index, i){
            let ok = if let Some(n) = target_note{
                (note - n as f32).abs() < 0.01
            }
            else { true };
            if ok{
                // line up with i so that (t + i) = (-i + i) = 0 is the first frame copied
                ts.push_back((-(i as i64), v));
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
}

#[allow(clippy::too_many_arguments)]
fn sample_lerp_gen(
    buf: &mut Sample, sb: &SampleBank, fb: &mut FlowwBank, len: usize, sample_index: usize,
    floww_index: usize, target_note: Option<usize>, lerp_len: usize, countdown: &mut usize,
    primary: &mut (i64, f32), ghost: &mut (i64, f32)
){
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
                // line up with i so that (t + i) = (-i + i) = 0 is the first frame copied
                *primary = (-(i as i64), v);
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
}

fn debug_sine_gen(
    buf: &mut Sample, fb: &mut FlowwBank, len: usize, floww_index: usize,
    notes: &mut Vec<(f32, f32)>, t: usize, sr: usize
){
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

#[allow(clippy::too_many_arguments)]
fn synth_gen(
    buf: &mut Sample, fb: &mut FlowwBank, len: usize, floww_index: usize,
    notes: &mut Vec<(f32, f32, f32, f32)>, square: &OscConf, topflat: &OscConf, triangle: &OscConf,
    t: usize, sr: usize
){
    let osc_amp_multiplier = 1.0 / (
        square.volume * square.adsr.max_vel() +
        topflat.volume * topflat.adsr.max_vel() +
        triangle.volume * triangle.adsr.max_vel());
    let mut release_sec = 0.0;
    if square.volume > 0.0 {
        release_sec = square.adsr.release_sec;
    }
    if topflat.volume > 0.0 {
        release_sec = release_sec.max(topflat.adsr.release_sec);
    }
    if triangle.volume > 0.0 {
        release_sec = release_sec.max(triangle.adsr.release_sec);
    }
    fb.start_block(floww_index);
    for i in 0..len{
        for (on, note, vel) in fb.get_block_simple(floww_index, i){
            if on{
                notes.push((note, vel, -(i as f32 / sr as f32), 0.0));
            } else {
                notes.retain(|x| (x.0 - note).abs() > 0.001 || x.3 == 0.0);
                for (n, _, env_t, rel_t) in notes.iter_mut(){
                    if (*n - note).abs() > 0.001 { continue; }
                    if *rel_t == 0.0{
                        *rel_t = *env_t + (i as f32 / sr as f32);
                        *env_t = -(i as f32 / sr as f32);
                    } else {
                        panic!("Synth: impossible release stage note");
                    }
                }
            }
        }

        buf.l[i] = 0.0;
        buf.r[i] = 0.0;
        for (note, vel, env_t, rel_t) in notes.iter(){
            let time = (t + i) as f32 / sr as f32;
            let env_time = env_t + (i as f32 / sr as f32);
            let hz = 440.0 * (2.0f32).powf((note - 69.0) / 12.0);

            let env_vel = |adsr_conf| if *rel_t == 0.0 { apply_ads(adsr_conf, env_time) }
            else { apply_r_rt(adsr_conf, env_time, *rel_t) };

            let mut s = 0.0;
            if square.volume > 0.0 {
                s += square_sine_sample(time, hz, square.param) * vel * env_vel(&square.adsr)
                    * square.volume;
            }
            if topflat.volume > 0.0 {
                s += topflat_sine_sample(time, hz, topflat.param) * vel * env_vel(&topflat.adsr)
                    * topflat.volume;
            }
            if triangle.volume > 0.0 {
                s += triangle_sample(time, hz) * vel * env_vel(&triangle.adsr) * triangle.volume;
            }
            s *= osc_amp_multiplier;
            buf.l[i] += s;
            buf.r[i] += s;
        }
    }
    for (_, _, env_t, _) in notes.iter_mut(){
        *env_t += len as f32 / sr as f32;
    }
    notes.retain(|x| x.3 == 0.0 || x.2 <= release_sec);
}

#[allow(clippy::too_many_arguments)]
fn sampsyn_gen(
    buf: &mut Sample, fb: &mut FlowwBank, len: usize, floww_index: usize,
    notes: &mut Vec<(f32, f32, f32, f32, WaveTableState)>, adsr: &AdsrConf, wave_table: &WaveTable,
    sr: usize
){
    let amp_multiplier = 1.0 / adsr.max_vel();
    fb.start_block(floww_index);
    for i in 0..len{
        for (on, note, vel) in fb.get_block_simple(floww_index, i){
            if on{
                let init_state = initial_state(wave_table, 0.0);
                notes.push((note, vel, -(i as f32 / sr as f32), 0.0, init_state));
            } else {
                notes.retain(|x| (x.0 - note).abs() > 0.001 || x.3 == 0.0);
                for (n, _, env_t, rel_t, _) in notes.iter_mut(){
                    if (*n - note).abs() > 0.001 { continue; }
                    if *rel_t == 0.0{
                        *rel_t = *env_t + (i as f32 / sr as f32);
                        *env_t = -(i as f32 / sr as f32);
                    } else {
                        panic!("Synth: impossible release stage note");
                    }
                }
            }
        }

        buf.l[i] = 0.0;
        buf.r[i] = 0.0;
        for (note, vel, env_t, rel_t, state) in notes.iter_mut(){
            let env_time = *env_t + (i as f32 / sr as f32);
            let hz = 440.0 * (2.0f32).powf((*note - 69.0) / 12.0);

            let env_vel = |adsr_conf| if *rel_t == 0.0 { apply_ads(adsr_conf, env_time) }
            else { apply_r_rt(adsr_conf, env_time, *rel_t) };

            let mut s = 0.0;
            let vel = *vel * env_vel(adsr) * amp_multiplier;
            s += wavetable_act_state(wave_table, state, hz, env_time + *rel_t, sr as f32) * vel;
            buf.l[i] += s;
            buf.r[i] += s;
        }
    }
    for (_, _, env_t, _, _) in notes.iter_mut(){
        *env_t += len as f32 / sr as f32;
    }
    notes.retain(|x| x.3 == 0.0 || x.2 <= adsr.release_sec);
}

#[cfg(feature = "lv2")]
fn lv2fx_gen(buf: &mut Sample, len: usize, wet: f32, index: usize, host: &mut Lv2Host){
    if wet < 0.0001 { return; }
    for i in 0..len{
        let ll = buf.l[i];
        let rr = buf.r[i];
        let (l, r) = host.apply(index, [0, 0, 0], (ll, rr));
        buf.l[i] = lerp(ll, l, wet);
        buf.r[i] = lerp(rr, r, wet);
    }
}

#[allow(clippy::too_many_arguments)]
fn adsr_gen(
    buf: &mut Sample, len: usize, fb: &mut FlowwBank, wet: f32, use_off: bool, use_max: bool,
    floww_index: usize, sr: usize, conf: &AdsrConf, note: Option<usize>,
    primary: &mut (f32, f32, f32), ghost: &mut (f32, f32, f32)
){
    if wet < 0.0001 { return; }
    let maxmul = if use_max { 1.0 } else { 0.0 };
    let minmul = 1.0 - maxmul;
    fb.start_block(floww_index);
    if use_off{
        for i in 0..len{
            let offset = i as f32 / sr as f32;
            for (on, n, v) in fb.get_block_simple(floww_index, i){
                if let Some(target) = note{
                    if (target as f32 - n).abs() > 0.01 { continue; }
                }
                if on{
                    *ghost = *primary;
                    *primary = (-(i as f32 / sr as f32), v, 0.0);
                } else if ghost.2 == 0.0 {
                    ghost.0 = -(i as f32 / sr as f32);
                    ghost.2 = apply_ads(conf, ghost.0 + offset) * ghost.1;
                } else {
                    primary.0 = -(i as f32 / sr as f32);
                    primary.2 = apply_ads(conf, primary.0 + offset) * primary.1;
                }
            }
            let pvel = if primary.2 == 0.0 { apply_ads(conf, primary.0 + offset) * primary.1 }
            else { apply_r(conf, primary.0 + offset, primary.2) * primary.1 };
            let gvel = if ghost.2 == 0.0 { apply_ads(conf, ghost.0 + offset) * ghost.1 }
            else { apply_r(conf, ghost.0 + offset, ghost.2) * ghost.1 };
            let adsr_vel = pvel.max(gvel) * maxmul + pvel.min(gvel) * minmul;
            let vel = lerp(1.0, adsr_vel, wet);

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
                *primary = (-(i as f32 / sr as f32), v, 0.0);
            }
            let offset = i as f32 / sr as f32;
            let pvel = apply_adsr(conf, primary.0 + offset) * primary.1;
            let gvel = apply_adsr(conf, ghost.0 + offset) * ghost.1;
            let adsr_vel = pvel.max(gvel) * maxmul + pvel.min(gvel) * minmul;
            let vel = lerp(1.0, adsr_vel, wet);

            buf.l[i] *= vel;
            buf.r[i] *= vel;
        }
    }
    primary.0 += len as f32 / sr as f32;
    ghost.0 += len as f32 / sr as f32;
}

#[allow(clippy::too_many_arguments)]
fn band_pass_gen(buf: &mut Sample, len: usize, wet: f32, first: &mut bool, pass: bool,
        lgamma: f32, hgamma: f32,
        lprevl: &mut f32, lprevr: &mut f32, hprevl: &mut f32, hprevr: &mut f32){
    if wet < 0.0001 { return; }
    if lgamma == 0.0 && hgamma == 0.0 { return; }
    let lmul = if lgamma == 0.0 { 0.0 } else { 1.0 };
    let hmul = if hgamma == 0.0 { 0.0 } else { 1.0 };
    let pass_mul = if pass { 1.0 } else { 0.0 };
    let cut_mul = 1.0 - pass_mul;

    if *first {
        *lprevl = buf.l[0];
        *lprevr = buf.r[0];
        *hprevl = buf.l[0];
        *hprevr = buf.r[0];
        *first = false;
    }
    for i in 0..len{
        let l = buf.l[i];
        let r = buf.r[i];
        let ll = *lprevl + lgamma * (l - *lprevl);
        let lr = *lprevr + lgamma * (r - *lprevr);
        let hl = *hprevl + hgamma * (l - *hprevl);
        let hr = *hprevr + hgamma * (r - *hprevr);
        *lprevl = ll;
        *lprevr = lr;
        *hprevl = hl;
        *hprevr = hr;
        let cutl = (lmul * ll + hmul * (l - hl)) * 0.5;
        let cutr = (lmul * lr + hmul * (r - hr)) * 0.5;
        let passl = l - cutl;
        let passr = r - cutl;
        buf.l[i] = cutl * cut_mul + passl * pass_mul;
        buf.r[i] = cutr * cut_mul + passr * pass_mul;
    }
}


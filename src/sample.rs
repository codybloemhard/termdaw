use rubato::{ Resampler, SincFixedIn, InterpolationType, InterpolationParameters, WindowFunction };
use term_basics_linux::UC;

use std::collections::{ HashMap, HashSet };

fn absmax(samples: &[f32]) -> f32{
    absmaxlen(samples, std::usize::MAX)
}

fn absmaxlen(samples: &[f32], len: usize) -> f32{
    samples.iter().take(len).fold(0.0, |max, s| { let a = s.abs(); if a > max { a } else { max } })
}

fn mean_energy(samples: &[f32]) -> f32{
    if samples.is_empty() {
        0.0
    } else {
        samples.iter().map(|s| s.abs()).sum::<f32>() / samples.len() as f32
    }
}

#[derive(Clone, Default, PartialEq, Debug)]
pub struct Sample{
    pub l: Vec<f32>,
    pub r: Vec<f32>,
}

impl Sample{
    pub fn new(bl: usize) -> Self{
        Self{
            l: vec![0.0; bl],
            r: vec![0.0; bl],
        }
    }

    pub fn from(l: Vec<f32>, r: Vec<f32>, method: SampleLoadMethod) -> Result<Self, String>{
        match method{
            SampleLoadMethod::Left => {
                if l.is_empty(){
                    return Err(format!("{r}TermDaw: Sample::from: l has length {b}0{r}.",
                        r = UC::Red, b = UC::Blue));
                }
                Ok(Self { l: l.clone(), r: l })
            },
            SampleLoadMethod::Right => {
                if r.is_empty(){
                    return Err(format!("{r}TermDaw: Sample::from: r has length {b}0{r}.",
                        r = UC::Red, b = UC::Blue));
                }
                Ok(Self { l: r.clone(), r })
            },
            SampleLoadMethod::Loudest => {
                let lm = mean_energy(&l);
                let rm = mean_energy(&r);
                if lm > rm{
                    Ok(Self { l: l.clone(), r: l })
                } else {
                    Ok(Self { l: r.clone(), r })
                }
            },
            _ => {
                if l.len() != r.len() {
                    return Err(format!("{r}TermDaw: Sample::from: l and r do not have the same length: {b}{llen}{r} and {b}{rlen}{r}.",
                        r = UC::Red, b = UC::Blue, llen = l.len(), rlen = r.len()));
                }
                if l.is_empty(){
                    return Err(format!("{r}TermDaw: Sample::from: l and r have length {b}0{r}.",
                        r = UC::Red, b = UC::Blue));
                }
                Ok(Self{ l, r })
            }
        }
    }

    pub fn len(&self) -> usize{
        self.l.len()
    }

    pub fn _is_empty(&self) -> bool{
        self.l.is_empty()
    }

    pub fn zero(&mut self){
        fn veczero(vec: &mut Vec<f32>){
            for v in vec{
                *v = 0.0;
            }
        }
        veczero(&mut self.l);
        veczero(&mut self.r);
    }

    pub fn apply_angle(&mut self, angle: f32, len: usize){
        if angle.abs() < 0.001 { return; }
        let angle_rad = angle * 0.5 * 0.01745329;
        let l_amp = std::f32::consts::FRAC_1_SQRT_2 * (angle_rad.cos() + angle_rad.sin());
        let r_amp = std::f32::consts::FRAC_1_SQRT_2 * (angle_rad.cos() - angle_rad.sin());
        for i in 0..len{
            self.l[i] *= l_amp;
            self.r[i] *= r_amp;
        }
    }

    pub fn apply_gain(&mut self, gain: f32, len: usize){
        if (gain - 1.0).abs() < 0.001 { return; }
        for i in 0..len.min(self.len()) {
            self.l[i] *= gain;
            self.r[i] *= gain;
        }
    }

    pub fn scan_max(&self, len: usize) -> f32{
        absmaxlen(&self.l, len).max(absmaxlen(&self.r, len))
    }

    pub fn scale(&mut self, len: usize, scalar: f32){
        self.l.iter_mut().take(len).for_each(|sample| *sample *= scalar);
        self.r.iter_mut().take(len).for_each(|sample| *sample *= scalar);
    }

    pub fn normalize(&mut self, len: usize){
        let len = len.min(self.len());
        let max = self.scan_max(len);
        let scalar = 1.0 / max;
        self.scale(len, scalar);
    }

    pub fn normalize_seperate(&mut self){
        let scalel = 1.0 / absmax(&self.l);
        let scaler = 1.0 / absmax(&self.r);
        self.l.iter_mut().for_each(|sample| *sample *= scalel);
        self.r.iter_mut().for_each(|sample| *sample *= scaler);
    }

    pub fn mix_down(&mut self){
        let l = std::mem::take(&mut self.l);
        let r = std::mem::take(&mut self.r);
        let mut mix = l.into_iter().zip(r).map(|(l,r)| l + r).collect::<Vec<_>>();
        let scale = 1.0 / absmax(&mix);
        mix.iter_mut().for_each(|s| *s *= scale);
        self.l.clone_from(&mix);
        self.r = mix;
    }

    //Not for chunks!
    pub fn resample(&self, from: usize, to: usize) -> Sample{
        // no idea what is means but comes from the example lol
        let params = InterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: InterpolationType::Nearest,
            oversampling_factor: 160,
            window: WindowFunction::BlackmanHarris2,
        };
        let mut resampler = SincFixedIn::<f32>::new(
            to as f64 / from as f64,
            params, self.len(), 2
        );
        let waves_in = vec![self.l.clone(), self.r.clone()];
        let mut waves_out = resampler.process(&waves_in).unwrap();
        let l = std::mem::take(&mut waves_out[0]);
        let r = std::mem::take(&mut waves_out[1]);
        Self{
            l, r
        }
    }

    pub fn interleave(self) -> Vec<f32>{
        let mut res = Vec::new();
        for i in 0..self.len(){
            res.push(self.l[i]);
            res.push(self.r[i]);
        }
        res
    }
}

pub struct SampleBank{
    sample_rate: usize,
    samples: Vec<Sample>,
    names: HashMap<String, usize>,
    max_sr: usize,
    max_bd: usize,
    marked: HashSet<usize>,
}

#[derive(Clone,Copy,PartialEq,Eq)]
pub enum SampleLoadMethod{ Stereo, Left, Right, Loudest, Norm, Mix }

impl SampleLoadMethod{
    pub fn from(string: &str) -> Self{
        match string{
            "left" => SampleLoadMethod::Left,
            "right" => SampleLoadMethod::Right,
            "loudest" => SampleLoadMethod::Loudest,
            "normalize-seperate" => SampleLoadMethod::Norm,
            "mix-down" => SampleLoadMethod::Mix,
            _ => SampleLoadMethod::Stereo,
        }
    }
}

impl SampleBank{
    pub fn new(sample_rate: usize) -> Self{
        Self{
            sample_rate,
            samples: Vec::new(),
            names: HashMap::new(),
            max_sr: 0,
            max_bd: 0,
            marked: HashSet::new(),
        }
    }

    pub fn add(&mut self, name: String, file: &str, method: SampleLoadMethod) -> Result<(), String>{
        if self.names.contains_key(&name) {
            return Err(format!("{r}TermDaw: SampleBank: there is already a sample with name {b}\"{n}\"{r} present.",
                r = UC::Red, b = UC::Blue, n = name));
        }
        let mut reader = if let Ok(reader) = hound::WavReader::open(file){
            reader
        } else {
            return Err(format!("{r}TermDaw: SampleBank: could not open file {b}\"{f}\"{r}.",
                r = UC::Red, b = UC::Blue, f = file));
        };
        let specs = reader.spec();
        if method == SampleLoadMethod::Stereo && specs.channels != 2{
            return Err(format!("{r}TermDaw: SampleBank: only 2 channel samples are supported for stereo samples, found {b}{s}{r} channels.",
                r = UC::Red, b = UC::Blue, s = specs.channels));
        }
        if method != SampleLoadMethod::Stereo && specs.channels > 2{
            return Err(format!("{r}TermDaw: SampleBank: only 1,2 channel samples are supported for left or right samples, found {b}{s}{r} channels.",
                r = UC::Red, b = UC::Blue, s = specs.channels));
        }
        let sr = specs.sample_rate as usize;
        let bd = specs.bits_per_sample;
        self.max_sr = self.max_sr.max(sr);
        self.max_bd = self.max_bd.max(bd as usize);
        if sr > self.sample_rate {
            println!("{y}TermDaw: warning: sample {b}\"{n}\"{y} has a higher samplerate({b}{sr}{y}) than the project({b}{psr}{y}).",
                y = UC::Yellow, b = UC::Blue, n = name, sr = sr, psr = self.sample_rate);
        }
        let mut linear = Vec::new();
        if specs.sample_format == hound::SampleFormat::Float{
            for s in reader.samples::<f32>(){
                if s.is_err() { continue; }
                linear.push(s.unwrap());
            }
        } else {
            //let max = ((1 << bd) / 2 - 1) as f32;
            for s in reader.samples::<i32>(){
                if s.is_err() { continue; }
                linear.push(s.unwrap() as f32);
            }
        }
        let mut l = Vec::new();
        let mut r = Vec::new();
        if specs.channels == 1{
            if method == SampleLoadMethod::Left{
                l = linear;
            } else {
                r = linear;
            }
        } else {
            let half_len = linear.len() /  2;
            for i in 0..half_len{
                l.push(linear[i * 2]);
                r.push(linear[i * 2 + 1]);
            }
            if linear.len() > half_len * 2{
                l.push(linear[linear.len() - 1]);
            }
        }
        let mut sample = match Sample::from(l, r, method){
            Ok(sample) => { sample },
            Err(e) => { return Err(e); }
        };
        if method == SampleLoadMethod::Norm{
            sample.normalize_seperate();
        } else if method == SampleLoadMethod::Mix{
            sample.mix_down();
        } else {
            sample.normalize(usize::MAX);
        }
        // resampling
        if sr != self.sample_rate{ // need to resample
            sample = sample.resample(sr, self.sample_rate);
        }
        self.samples.push(sample);
        self.names.insert(name, self.samples.len() - 1);
        Ok(())
    }

    pub fn mark_dead(&mut self, name: &str){
        if let Some(index) = self.names.get(name){
            self.marked.insert(*index);
        }
    }

    pub fn refresh(&mut self){
        if self.marked.is_empty() { return; }
        let mut new_map = HashMap::new();
        let mut new_vec = Vec::new();
        let names = std::mem::take(&mut self.names);
        for (name, index) in names{
            if self.marked.contains(&index) { continue; }
            let sample = std::mem::take(&mut self.samples[index]);
            new_vec.push(sample);
            new_map.insert(name, new_vec.len() - 1);
        }
        self.names = new_map;
        self.samples = new_vec;
        self.marked.clear();
    }

    pub fn get_index(&self, name: &str) -> Option<usize>{
        self.names.get(name).copied()
    }

    pub fn get_sample(&self, index: usize) -> &Sample{
        &self.samples[index]
    }

    pub fn get_max_sr_bd(&self) -> (usize, usize){
        (self.max_sr, self.max_bd)
    }
}


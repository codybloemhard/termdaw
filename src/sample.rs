use std::collections::{ HashMap };

use rubato::{ Resampler, SincFixedIn, InterpolationType, InterpolationParameters, WindowFunction };

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

    pub fn from(l: Vec<f32>, r: Vec<f32>) -> Result<Self, String>{
        if l.len() != r.len() {
            return Err(format!("TermDaw: Sample::from: l and r do not have the same length: {} and {}.", l.len(), r.len()));
        }
        if l.is_empty(){
            return Err("TermDaw: Sample::from: l and r have length 0.".to_owned());
        }
        Ok(Self{ l, r })
    }

    pub fn len(&self) -> usize{
        self.l.len()
    }

    pub fn is_empty(&self) -> bool{
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
        let max = self.l.iter().take(len).map(|s| s.abs()).fold(0.0, |max, s| if s > max { s } else { max });
        self.r.iter().take(len).map(|s| s.abs()).fold(0.0, |max, s| if s > max { s } else { max }).max(max)
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
        let l = std::mem::replace(&mut waves_out[0], Vec::new());
        let r = std::mem::replace(&mut waves_out[1], Vec::new());
        Self{
            l, r
        }
    }
}

pub struct SampleBank{
    sample_rate: usize,
    samples: Vec<Sample>,
    names: HashMap<String, usize>,
    max_sr: usize,
    max_bd: usize,
}

impl SampleBank{
    pub fn new(sample_rate: usize) -> Self{
        Self{
            sample_rate,
            samples: Vec::new(),
            names: HashMap::new(),
            max_sr: 0,
            max_bd: 0,
        }
    }

    pub fn add(&mut self, name: String, file: &str) -> Result<(), String>{
        if self.names.get(&name).is_some() {
            return Err(format!("TermDaw: SampleBank: there is already an sample with name \"{}\" present.", name));
        }
        let mut reader = if let Ok(reader) = hound::WavReader::open(file){
            reader
        } else {
            return Err(format!("TermDaw: SampleBank: could not open file {}.", file));
        };
        let specs = reader.spec();
        if specs.channels != 2 {
            return Err(format!("TermDaw: SampleBank: only stereo samples are supported yet, found {} channels.", specs.channels));
        }
        let sr = specs.sample_rate as usize;
        let bd = specs.bits_per_sample;
        self.max_sr = self.max_sr.max(sr);
        self.max_bd = self.max_bd.max(bd as usize);
        if sr > self.sample_rate {
            println!("TermDaw: warning: sample \"{}\" has a higher samplerate({}) than the project({}).", name, sr, self.sample_rate);
        }
        let mut l = Vec::new();
        let mut r = Vec::new();
        let mut c = 0;
        if specs.sample_format == hound::SampleFormat::Float{
            for s in reader.samples::<f32>(){
                if s.is_err() { continue; }
                let s = s.unwrap();
                if c == 0 {
                    l.push(s);
                    c = 1;
                } else {
                    r.push(s);
                    c = 0;
                }
            }
        } else {
            let max = ((1 << bd) / 2 - 1) as f32;
            for s in reader.samples::<i32>(){
                if s.is_err() { continue; }
                let s = s.unwrap() as f32 / max;
                if c == 0 {
                    l.push(s);
                    c = 1;
                } else {
                    r.push(s);
                    c = 0;
                }
            }
        }
        let mut sample = match Sample::from(l, r){
            Ok(sample) => { sample },
            Err(e) => { return Err(e); }
        };
        sample.normalize(usize::MAX);
        // resampling
        if sr != self.sample_rate{ // need to resample
            sample = sample.resample(sr, self.sample_rate);
        }
        self.samples.push(sample);
        self.names.insert(name, self.samples.len() - 1);
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&Sample>{
        if let Some(index) = self.names.get(name){
            Some(&self.samples[*index])
        } else {
            None
        }
    }

    pub fn get_max_sr_bd(&self) -> (usize, usize){
        (self.max_sr, self.max_bd)
    }
}


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
}

pub struct SampleBank{
    sample_rate: usize,
    samples: Vec<Sample>,
    names: HashMap<String, usize>,
}

impl SampleBank{
    pub fn new(sample_rate: usize) -> Self{
        Self{
            sample_rate,
            samples: Vec::new(),
            names: HashMap::new(),
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
        if sr != self.sample_rate{ // need to resample
            // no idea what is means but comes from the example lol
            let params = InterpolationParameters {
                sinc_len: 256,
                f_cutoff: 0.95,
                interpolation: InterpolationType::Nearest,
                oversampling_factor: 160,
                window: WindowFunction::BlackmanHarris2,
            };
            let mut resampler = SincFixedIn::<f32>::new(
                self.sample_rate as f64 / sr as f64,
                params, l.len(), 2
            );
            let waves_in = vec![l, r];
            let mut waves_out = resampler.process(&waves_in).unwrap();
            l = std::mem::replace(&mut waves_out[0], Vec::new());
            r = std::mem::replace(&mut waves_out[1], Vec::new());
        }
        match Sample::from(l, r){
            Ok(sample) => {
                self.samples.push(sample);
                self.names.insert(name, self.samples.len() - 1);
                Ok(())
            },
            Err(e) => {
                Err(e)
            }
        }
    }

    pub fn get_sample(&self, name: &str) -> Option<&Sample>{
        if let Some(index) = self.names.get(name){
            Some(&self.samples[*index])
        } else {
            None
        }
    }
}


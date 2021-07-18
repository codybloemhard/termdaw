
#[derive(Clone, Copy)]
pub struct AdsrConf{
    pub std_vel: f32,
    pub attack_sec: f32,
    pub attack_vel: f32,
    pub decay_sec: f32,
    pub decay_vel: f32,
    pub sustain_sec: f32,
    pub sustain_vel: f32,
    pub release_sec: f32,
    pub release_vel: f32,
}

pub fn hit_adsr_conf(attack_sec: f32, decay_sec: f32, decay_vel: f32, sustain_sec: f32, sustain_vel: f32, release_sec: f32) -> AdsrConf{
    AdsrConf{
        std_vel: 0.0,
        attack_sec,
        attack_vel: 1.0,
        decay_sec,
        decay_vel,
        sustain_sec,
        sustain_vel,
        release_vel: 0.0,
        release_sec,
    }
}

fn apply_ads_internal(conf: &AdsrConf, t: f32) -> f32{
    if t <= conf.attack_sec {
        conf.std_vel + (conf.attack_vel - conf.std_vel) * (t / conf.attack_sec)
    } else if t <= conf.attack_sec + conf.decay_sec{
        conf.attack_vel + (conf.decay_vel - conf.attack_vel) * ((t - conf.attack_sec) / conf.decay_sec)
    } else if t <= conf.attack_sec + conf.decay_sec + conf.sustain_sec{
        conf.decay_vel + (conf.sustain_vel - conf.decay_vel) * ((t - conf.attack_sec - conf.decay_sec) / conf.sustain_sec)
    } else {
        -1000.0
    }
}

pub fn apply_ads(conf: &AdsrConf, t: f32) -> f32{
    let res = apply_ads_internal(conf, t);
    if res <= -1.0{
        conf.sustain_vel
    } else {
        res
    }
}

pub fn apply_r(conf: &AdsrConf, t: f32) -> f32{
    conf.sustain_vel + (conf.release_vel - conf.sustain_vel) * ((t - conf.attack_sec - conf.decay_sec - conf.sustain_sec) / conf.release_sec).min(1.0)
}

pub fn apply_adsr(conf: &AdsrConf, t: f32) -> f32{
    let res = apply_ads_internal(conf, t);
    if res <= -1.0{
        apply_r(conf, t)
    } else {
        res
    }
}


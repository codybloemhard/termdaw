
#[derive(Clone, Copy)]
pub struct AdsrConf{
    pub std_vel: f32,
    pub attack_ms: f32,
    pub attack_vel: f32,
    pub decay_ms: f32,
    pub decay_vel: f32,
    pub sustain_ms: f32,
    pub sustain_vel: f32,
    pub release_ms: f32,
    pub release_vel: f32,
}

pub fn hit_adsr_conf(attack_ms: f32, decay_ms: f32, decay_vel: f32, sustain_ms: f32, sustain_vel: f32, release_ms: f32) -> AdsrConf{
    AdsrConf{
        std_vel: 0.0,
        attack_ms,
        attack_vel: 1.0,
        decay_ms,
        decay_vel,
        sustain_ms,
        sustain_vel,
        release_vel: 0.0,
        release_ms,
    }
}

fn apply_ads_internal(conf: &AdsrConf, t: f32) -> f32{
    if t <= conf.attack_ms {
        conf.std_vel + (conf.attack_vel - conf.std_vel) * (t / conf.attack_ms)
    } else if t <= conf.attack_ms + conf.decay_ms{
        conf.attack_vel + (conf.decay_vel - conf.attack_vel) * ((t - conf.attack_ms) / conf.decay_ms)
    } else if t <= conf.attack_ms + conf.decay_ms + conf.sustain_ms{
        conf.decay_vel + (conf.sustain_vel - conf.decay_vel) * ((t - conf.attack_ms - conf.decay_ms) / conf.sustain_ms)
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
    conf.sustain_vel + (conf.release_vel - conf.sustain_vel) * ((t - conf.attack_ms - conf.decay_ms - conf.sustain_ms) / conf.release_ms)
}

pub fn apply_adsr(conf: &AdsrConf, t: f32) -> f32{
    let res = apply_ads_internal(conf, t);
    if res <= -1.0{
        apply_r(conf, t)
    } else {
        res
    }
}


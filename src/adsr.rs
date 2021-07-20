
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
    conf.sustain_vel + (conf.release_vel - conf.sustain_vel) * (t / conf.release_sec).min(1.0)
}

pub fn apply_adsr(conf: &AdsrConf, t: f32) -> f32{
    let res = apply_ads_internal(conf, t);
    if res <= -1.0{
        conf.sustain_vel + (conf.release_vel - conf.sustain_vel) * ((t - conf.attack_sec - conf.decay_sec - conf.sustain_sec) / conf.release_sec).min(1.0)
    } else {
        res
    }
}

#[cfg(test)]
mod tests{
    use crate::adsr::*;
    #[test]
    fn adsr_0(){
        let conf = hit_adsr_conf(1.0, 1.0, 0.5, 1.0, 0.25, 1.0);
        assert!(apply_adsr(&conf, 0.0).abs() < 0.001);
        assert!((0.5   - apply_adsr(&conf, 0.5)).abs() < 0.001);
        assert!((1.0   - apply_adsr(&conf, 1.0)).abs() < 0.001);
        assert!((0.75  - apply_adsr(&conf, 1.5)).abs() < 0.001);
        assert!((0.5   - apply_adsr(&conf, 2.0)).abs() < 0.001);
        assert!((0.375 - apply_adsr(&conf, 2.5)).abs() < 0.001);
        assert!((0.25  - apply_adsr(&conf, 3.0)).abs() < 0.001);
        assert!((0.125 - apply_adsr(&conf, 3.5)).abs() < 0.001);
        assert!((0.0   - apply_adsr(&conf, 4.0)).abs() < 0.001);
        assert!((0.0   - apply_adsr(&conf, 8.0)).abs() < 0.001);
    }

    #[test]
    fn adsr_1(){
        let conf = hit_adsr_conf(1.0, 1.0, 0.5, 1.0, 0.25, 1.0);
        assert!(apply_adsr(&conf, 0.0).abs() < 0.001);
        assert!((0.5   - apply_ads(&conf, 0.5)).abs() < 0.001);
        assert!((1.0   - apply_ads(&conf, 1.0)).abs() < 0.001);
        assert!((0.75  - apply_ads(&conf, 1.5)).abs() < 0.001);
        assert!((0.5   - apply_ads(&conf, 2.0)).abs() < 0.001);
        assert!((0.375 - apply_ads(&conf, 2.5)).abs() < 0.001);
        assert!((0.25  - apply_ads(&conf, 3.0)).abs() < 0.001);
        assert!((0.25  - apply_ads(&conf, 7.0)).abs() < 0.001);
        assert!((0.25  - apply_r(&conf, 0.0)).abs() < 0.001);
        assert!((0.125 - apply_r(&conf, 0.5)).abs() < 0.001);
        assert!((0.0   - apply_r(&conf, 1.0)).abs() < 0.001);
        assert!((0.0   - apply_r(&conf, 9.0)).abs() < 0.001);
    }
}

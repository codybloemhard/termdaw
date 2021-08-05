use std::f32::consts::PI;
use crate::adsr::AdsrConf;

pub type OscConf = (f32, f32, AdsrConf);

#[inline]
pub fn square_sine_sample(t: f32, hz: f32, z: f32) -> f32{
    (t * hz * 2.0 * PI).sin().max(-z).min(z) * (1.0 / z)
}

#[inline]
pub fn topflat_sine_sample(t: f32, hz: f32, z: f32) -> f32{
    ((t * hz * 2.0 * PI).sin().min(z) + ((1.0 - z) / 2.0)) * (2.0 / (1.0 + z))
}

#[inline]
pub fn triangle_sample(t: f32, hz: f32) -> f32{
    4.0 * ((t * hz) - ((t * hz) + 0.5).floor()).abs() - 1.0
}

// formula's i made for the square_sine and topflat_sine oscilators
// https://graphtoy.com/?f1(x,t)=min(sin(x),0)*2+1&v1=false&f2(x,t)=max(sin(x),0)*2-1&v2=false&f3(x,t)=0.4&v3=true&f4(x,t)=(min(sin(x),f3(0))+((1-f3(0))/2))*(2/(1+f3(0)))&v4=false&f5(x,t)=(max(sin(x),-f3(0))-((1-f3(0))/2))*(2/(1+f3(0)))&v5=false&f6(x,t)=clamp(sin(x),%20-f3(0),%20f3(0))%20*%20(1%20/%20f3(0))&v6=true&grid=true&coords=0,0,4.205926793776712

use std::f32::consts::PI;

#[inline]
pub fn square_sine_sample(t: f32, hz: f32, z: f32) -> f32{
    (t * hz * 2.0 * PI).sin().max(-z).min(z)
}

#[inline]
pub fn triangle_sample(t: f32, hz: f32) -> f32{
    4.0 * ((t * hz) - ((t * hz) + 0.5).floor()).abs() - 1.0
}

#[inline]
pub fn topflat_sine_sample(t: f32, hz: f32, z: f32) -> f32{
    ((t * hz * 2.0 * PI).sin().min(z) + ((1.0 - z) / 2.0)) * (2.0 / (1.0 + z))
}


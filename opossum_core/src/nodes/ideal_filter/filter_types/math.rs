/// Calculates a smooth transition (Smootherstep) between 0.0 and 1.0.
/// This function has is 2nd derivative zero at the borders.
pub fn smootherstep(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    // Ken Perlin's smootherstep: 6x^5 - 15x^4 + 10x^3
    x * x * x * (x * (x * 6.0 - 15.0) + 10.0)
}

/// Interpolates between 2 Trnasmission values.
/// Makes sure that the value exactly reaches `start` or `end` if the borders are reached.
pub fn interpolate_transition(x: f64, start: f64, end: f64) -> f64 {
    let t = smootherstep(x);
    if t <= 1e-15 {
        start
    } else if t > (1.0 - 1e-15) {
        end
    } else {
        t.mul_add(end - start, start)
    }
}

// filter_types/math.rs

/// Berechnet einen glatten Übergang (Smootherstep) zwischen 0.0 und 1.0.
/// Diese Funktion ist C2-kontinuierlich (erste und zweite Ableitung sind an den Grenzen 0).
pub fn smootherstep(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    // Ken Perlin's smootherstep: 6x^5 - 15x^4 + 10x^3
    x * x * x * (x * (x * 6.0 - 15.0) + 10.0)
}

/// Hilfsfunktion für den Übergang zwischen zwei Transmissionswerten.
pub fn interpolate_transition(x: f64, start_val: f64, end_val: f64) -> f64 {
    let t = smootherstep(x);
    t.mul_add(end_val - start_val, start_val)
}

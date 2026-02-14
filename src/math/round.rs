
pub fn round(x: f64, digits: u32) -> f64 {
    let pow1: f64;
    let pow2: f64;

    if digits > 22 {
            /* pow1 and pow2 are each safe from overflow, but
               pow1*pow2 ~= pow(10.0, ndigits) might overflow */
        pow1 = (10.0 as f64).powi((digits - 22) as i32);
        pow2 = 1e22;
    }
    else {
        pow1 = (10.0 as f64).powi(digits as i32);
        pow2 = 1.0;
    }
        
    let y = (x * pow1) * pow2;

    let mut z = y.round();

    if (y-z).abs() == 0.5 {
        z = 2.0 * ((y / 2.0).round() as f64)
    }
    
    (z / pow2) / pow1
}
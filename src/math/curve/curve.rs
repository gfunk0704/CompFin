

pub trait Curve {
    fn value(&self, x: f64) -> f64;

    fn derivative(&self, x: f64) -> f64;
}
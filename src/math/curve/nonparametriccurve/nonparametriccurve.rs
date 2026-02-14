
pub struct Point2D {
    x: f64,
    y: f64
}

impl Point2D {
    pub fn new(x: f64, y: f64) -> Point2D {
        Point2D { x: x, y: y }
    }

    pub fn x(&self) -> f64 {
        self.x
    }

    pub fn y(&self) -> f64 {
        self.y
    }

    pub fn slope(lhs_pt: &Point2D, rhs_pt: &Point2D) -> f64 {
        (rhs_pt.y - lhs_pt.y) / (rhs_pt.x - lhs_pt.x)
    }
}

pub trait NonparametricCurve {
    fn points(&self) -> Vec<Point2D>;

    fn min_x(&self) -> f64;

    fn max_x(&self) -> f64;
}
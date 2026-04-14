use crate::math::curve::curve::Curve;


pub struct Point2D {
    x: f64,
    y: f64,
}

impl Point2D {
    pub fn new(x: f64, y: f64) -> Point2D {
        Point2D { x, y }
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


/// 所有 nonparametric curve 都是 `Curve`，
/// 額外提供描述資料形狀的方法。
pub trait NonparametricCurve: Curve {
    fn points(&self) -> Vec<Point2D>;
    fn min_x(&self) -> f64;
    fn max_x(&self) -> f64;
}

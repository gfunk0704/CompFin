use serde::Deserialize;


#[derive(PartialEq, Eq, Clone, Copy, Deserialize)]
pub enum GenerationDirection {
    Forward = 1,
    Backward = -1
}

use serde::{
    Serialize,
    Deserialize
};

#[derive(Clone, Serialize, Deserialize)]
pub struct NamedJsonObject {
    name: String
}


impl NamedJsonObject {
    pub fn new(name: String) -> NamedJsonObject {
        NamedJsonObject { name: name }
    } 

    pub fn name(&self) -> &String {
        &self.name
    }
}

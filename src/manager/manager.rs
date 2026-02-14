use std::cell::{
    RefCell, RefMut
};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;

use serde_json;

use super::managererror::ManagerError;
use super::namedobject::NamedJsonObject;


pub trait IManager<V, S> where 
    V: Clone {
    fn map(&self) -> RefMut<'_, HashMap<String, V>>;
    
    fn insert_obj_from_json(&self, 
                            json_value: serde_json::Value,
                            supports: &S) -> Result<(), ManagerError>;

    fn get(&self, name: &String) -> Result<V, ManagerError> {
        let map = self.map();
        let elem_opt = map.get(name);
        elem_opt.map_or(
            Err(ManagerError::NameNotFoundError(name.to_owned())), 
            |elem| Ok(elem.clone())
        )
    }

    fn insert_obj_from_json_vec(&self, 
                                json_vec: &Vec<serde_json::Value>,
                                supports: &S) -> Result<(), ManagerError> {                
        for j in json_vec.iter() {
            let _ = self.insert_obj_from_json(j.clone(), &supports)?;
        }
        Ok(())
    }

    fn from_reader(&self, 
                   file_path: String,
                   supports: &S) -> Result<(), ManagerError> {
        let file = File::open(file_path).map_err(|error| ManagerError::IOError(error))?;
        let reader = BufReader::new(file);
        let json_value: serde_json::Value = serde_json::from_reader(reader).map_err(|error| ManagerError::JsonParseError(error))?;
        if json_value.is_array() {
            let json_array: Vec<serde_json::Value> = ManagerError::from_json_or_json_parse_error(json_value)?;
            let _ = self.insert_obj_from_json_vec(&json_array, supports)?;
            
        } else {
            let _ = self.insert_obj_from_json(json_value, supports);
        }
        Ok(())
    }
}


pub struct Manager<V> {
    map_cell: RefCell<HashMap<String, V>>,
    get_obj_from_json: fn(serde_json::Value) -> Result<V, ManagerError>
}


impl <V> Manager<V> where 
    V: Clone {
    pub fn new(get_obj_from_json: fn(serde_json::Value) -> Result<V, ManagerError>) -> Manager<V> {
        Manager {map_cell: RefCell::new(HashMap::new()), get_obj_from_json}
    }
}

impl <V> IManager<V, ()> for Manager<V> where 
    V: Clone {
    fn map(&self) -> RefMut<'_, HashMap<String, V>> {
        self.map_cell.borrow_mut()
    }

    fn insert_obj_from_json(&self, 
                            json_value: serde_json::Value,
                            _supports: &()) -> Result<(), ManagerError> {   
        let named_object: NamedJsonObject = ManagerError::from_json_or_json_parse_error(json_value.clone())?; 
        let v = (self.get_obj_from_json)(json_value)?;
        self.map().insert(named_object.name().to_owned(), v);
        Ok(())
    }
} 


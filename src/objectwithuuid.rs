use uuid::Uuid;

pub trait ObjectWithUUID {
    fn uuid(&self) -> &Uuid;
}
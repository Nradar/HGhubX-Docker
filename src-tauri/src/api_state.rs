use crate::session::SessionStore;

pub struct Api {
    pub session: SessionStore,
}

impl Api {
    pub fn new() -> Self {
        Self {
            session: SessionStore::new(),
        }
    }
}

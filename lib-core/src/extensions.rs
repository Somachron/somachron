use std::sync::Arc;

#[repr(transparent)]
pub struct ReqId(pub Arc<str>);
impl Clone for ReqId {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[repr(transparent)]
pub struct UserId(pub Arc<str>);
impl Clone for UserId {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[derive(Clone, Copy)]
pub enum UserRole {
    Owner,
    Read,
    Upload,
    Modify,
}

pub struct SpaceCtx {
    pub id: Arc<str>,
    pub role: UserRole,
}
impl Clone for SpaceCtx {
    fn clone(&self) -> Self {
        Self {
            id: Arc::clone(&self.id),
            role: self.role,
        }
    }
}

#[repr(transparent)]
pub struct ReqId(pub String);
impl Clone for ReqId {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[repr(transparent)]
pub struct UserId(pub String);
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
    pub id: String,
    pub role: UserRole,
}
impl Clone for SpaceCtx {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            role: self.role,
        }
    }
}

use lib_core::clerk::TokenClaims;
use uuid::Uuid;

use crate::datastore::user_space::SpaceRole;

#[repr(transparent)]
#[derive(Clone)]
pub struct Claims(pub TokenClaims);

#[repr(transparent)]
pub struct UserId(pub Uuid);
impl Clone for UserId {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

pub struct SpaceCtx {
    pub membership_id: Uuid,
    pub space_id: Uuid,
    pub role: SpaceRole,
}
impl Clone for SpaceCtx {
    fn clone(&self) -> Self {
        Self {
            membership_id: self.membership_id.clone(),
            space_id: self.space_id.clone(),
            role: self.role,
        }
    }
}

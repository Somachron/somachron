use lib_core::clerk::TokenClaims;
use surrealdb::RecordId;

use crate::datastore::user_space::SpaceRole;

#[repr(transparent)]
#[derive(Clone)]
pub struct Claims(pub TokenClaims);

#[repr(transparent)]
pub struct UserId(pub RecordId);
impl Clone for UserId {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

pub struct SpaceCtx {
    pub membership_id: RecordId,
    pub space_id: RecordId,
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

pub trait IdStr {
    fn id(&self) -> String;
}

impl IdStr for RecordId {
    fn id(&self) -> String {
        format!("{}", self.key())
    }
}

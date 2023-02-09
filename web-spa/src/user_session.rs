use serde::{Deserialize, Serialize};
use uuid::Uuid;
use yewdux::prelude::*;

use crate::api::auth_api::LoginResponse;

#[derive(Debug, Clone, PartialEq, Default, Store, Serialize, Deserialize)]
#[store(storage = "local")]
pub struct UserSession {
    pub user_id: Uuid,
    pub email: String,
    pub token: String,
}

impl UserSession {
    pub fn logged(&self) -> bool {
        !self.token.is_empty()
    }

    pub fn login(dispatch: Dispatch<UserSession>, session: LoginResponse) {
        dispatch.reduce_mut(move |store| {
            store.user_id = session.user_id;
            store.email = session.email;
            store.token = session.access_token;
        })
    }

    pub fn logout(dispatch: Dispatch<UserSession>) {
        dispatch.set(Default::default())
    }
}

use diesel::Queryable;
use serde::ser::{SerializeStruct, Serializer};
use serde::{Deserialize, Serialize};

use crate::schema::{matches, swipes, users};

#[derive(Queryable, Insertable, Deserialize)]
#[table_name = "users"]
pub struct DBUser {
    pub(crate) username: String,
    pub(crate) password: String,
    pub(crate) lat: Option<f64>,
    pub(crate) long: Option<f64>,
    pub(crate) bio: Option<String>,
    pub(crate) profile_pic: Option<String>,
}

#[derive(Queryable, Insertable, Debug)]
#[table_name = "swipes"]
pub struct DBSwipe {
    pub(crate) swiper: String,
    pub(crate) swiped: String,
    pub(crate) status: bool,
}

#[derive(Queryable, Insertable, Debug)]
#[table_name = "matches"]
pub struct DBMatch {
    pub(crate) username1: String,
    pub(crate) username2: String,
}

impl Serialize for DBUser {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("DBUser", 4)?;
        state.serialize_field("username", &self.username)?;
        state.serialize_field("lat", &self.lat)?;
        state.serialize_field("long", &self.long)?;
        state.serialize_field("bio", &self.bio)?;
        state.end()
    }
}

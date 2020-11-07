use std::env;

use diesel::r2d2::Pool;
use diesel::{pg::PgConnection, r2d2::ConnectionManager, Queryable};
use serde::ser::{Serialize, SerializeStruct, Serializer};

#[derive(Queryable)]
pub struct DBUser {
    pub(crate) username: String,
    pub(crate) password: String,
    pub(crate) lat: Option<f64>,
    pub(crate) long: Option<f64>,
}

#[derive(Queryable)]
pub struct Swipe {
    pub(crate) swiper: String,
    pub(crate) swiped: String,
    pub(crate) status: bool,
}

impl Serialize for DBUser {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("DBUser", 3)?;
        state.serialize_field("username", &self.username)?;
        state.serialize_field("latitude", &self.lat)?;
        state.serialize_field("longitude", &self.long)?;
        state.end()
    }
}

pub fn connection_pool() -> Result<Pool<ConnectionManager<PgConnection>>, String> {
    dotenv::dotenv().ok();
    let database_url = env::var("DATABASE_URL").map_err(|e| e.to_string())?;
    let manager = ConnectionManager::<PgConnection>::new(&database_url);
    Pool::builder()
        .max_size(3)
        .build(manager)
        .map_err(|err| err.to_string())
}

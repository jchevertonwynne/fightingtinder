use std::env;

use crate::schema::users;
use diesel::r2d2::Pool;
use diesel::{
    pg::PgConnection, r2d2::ConnectionManager, ExpressionMethods, QueryDsl, Queryable, RunQueryDsl,
};
use serde::ser::{Serialize, SerializeStruct, Serializer};

#[derive(Queryable)]
pub struct DBUser {
    pub(crate) id: i32,
    pub(crate) name: String,
    pub(crate) password: String,
}

impl Serialize for DBUser {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("DBUser", 1)?;
        state.serialize_field("name", &self.name)?;
        state.end()
    }
}

impl DBUser {
    pub fn find(id: i32, conn: &PgConnection) -> Result<DBUser, String> {
        match users::dsl::users
            .filter(users::dsl::id.eq(&id))
            .limit(1)
            .load::<DBUser>(conn)
        {
            Ok(mut db_users) => match db_users.pop() {
                Some(user) => Ok(user),
                None => Err(format!("user with id `{}` could not be found", id)),
            },
            Err(err) => Err(err.to_string()),
        }
    }
}

pub fn connection_pool() -> Result<Pool<ConnectionManager<PgConnection>>, String> {
    dotenv::dotenv().ok();
    let database_url = env::var("DATABASE_URL").map_err(|e| e.to_string())?;
    let manager = diesel::r2d2::ConnectionManager::<PgConnection>::new(&database_url);
    Pool::builder()
        .max_size(3)
        .build(manager)
        .map_err(|err| err.to_string())
}

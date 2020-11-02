use std::env;

use diesel::{r2d2::ConnectionManager, ExpressionMethods, QueryDsl, Queryable, RunQueryDsl, pg::PgConnection};

use crate::schema::users::dsl::{users, id};
use diesel::r2d2::Pool;

#[derive(Queryable)]
pub struct DBUser {
    pub(crate) id: i32,
    pub(crate) name: String,
    pub(crate) password: String,
}

impl DBUser {
    pub fn find(s: &str, conn: &PgConnection) -> Result<DBUser, String> {
        match s.parse::<i32>() {
            Ok(i) => {
                match users.filter(id.eq(i)).limit(1).load::<DBUser>(conn) {
                    Ok(mut db_users) => {
                        match db_users.pop() {
                            Some(user) => Ok(user),
                            None => Err(format!("user with id `{}` could not be found", s))
                        }
                    }
                    Err(err) => Err(err.to_string())
                }
            }
            Err(err) => Err(err.to_string())
        }
    }
}

pub fn connection_pool() -> Result<Pool<ConnectionManager<PgConnection>>, String> {
    dotenv::dotenv().ok();
    let database_url = env::var("DATABASE_URL").map_err(|e| e.to_string())?;
    let manager = diesel::r2d2::ConnectionManager::<PgConnection>::new(&database_url);
    Pool::builder().max_size(3).build(manager).map_err(|err| err.to_string())
}
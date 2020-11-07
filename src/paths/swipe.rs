use std::sync::Arc;

use actix_web::{get, web, HttpResponse, Responder, Scope};
use diesel::{
    r2d2::{ConnectionManager, Pool},
    BoolExpressionMethods, ExpressionMethods, PgConnection, QueryDsl, RunQueryDsl,
};

use crate::db::DBUser;
use crate::schema::swipes;
use crate::schema::users;

pub fn create_paths() -> Scope {
    web::scope("/swipe").service(available)
}

#[get("/available")]
async fn available(
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    match conn_pool.as_ref().try_get() {
        Some(conn) => {
            let not_swiped_on = users::dsl::users.filter(
                diesel::dsl::not(diesel::dsl::exists(
                    swipes::dsl::swipes
                        .filter(swipes::swiper.eq("joe"))
                        .filter(swipes::swiped.eq(users::username)),
                ))
                .and(users::username.ne("joe")),
            );

            match not_swiped_on.load::<DBUser>(&conn) {
                Ok(users) => match serde_json::to_string(&users) {
                    Ok(users_string) => HttpResponse::Ok().body(users_string),
                    Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
                },
                Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
            }
        }
        None => HttpResponse::InternalServerError().body("couldnt get a db connection"),
    }
}

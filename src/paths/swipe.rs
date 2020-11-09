use std::sync::Arc;

use actix_web::{web, HttpRequest, HttpResponse, Responder};
use diesel::{
    r2d2::{ConnectionManager, Pool},
    BoolExpressionMethods, ExpressionMethods, Insertable, PgConnection, QueryDsl, RunQueryDsl,
};
use serde::{Deserialize, Serialize};

use crate::db::{DBMatch, DBSwipe, DBUser};
use crate::schema::matches;
use crate::schema::swipes;
use crate::schema::users;

#[derive(Insertable, Serialize, Deserialize, Debug)]
#[table_name = "swipes"]
pub struct SwipeDTO {
    swiped: String,
    status: bool,
}

pub async fn available(
    req: HttpRequest,
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    let extensions = req.extensions();
    let username = extensions
        .get::<String>()
        .expect("middleware should have set username");

    match conn_pool.as_ref().try_get() {
        Some(conn) => {
            let not_swiped_on = users::dsl::users.filter(
                diesel::dsl::not(diesel::dsl::exists(
                    swipes::dsl::swipes
                        .filter(swipes::swiper.eq(&username))
                        .filter(swipes::swiped.eq(users::username)),
                ))
                .and(users::username.ne(&username)),
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

pub async fn do_swipe(
    swipe: web::Json<SwipeDTO>,
    req: HttpRequest,
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    let extensions = req.extensions();
    let swiper = extensions
        .get::<String>()
        .expect("username should be got by middleware");
    match conn_pool.as_ref().try_get() {
        Some(conn) => {
            let swipe = DBSwipe {
                swiper: swiper.clone(),
                swiped: swipe.swiped.clone(),
                status: swipe.status,
            };
            match diesel::insert_into(swipes::table)
                .values(&swipe)
                .get_result::<DBSwipe>(&conn)
            {
                Ok(_) => {
                    match swipes::dsl::swipes
                        .filter(swipes::swiper.eq(&swipe.swiped))
                        .filter(swipes::swiped.eq(&swiper))
                        .filter(swipes::status.eq(true))
                        .first::<DBSwipe>(&conn)
                    {
                        Ok(_) => {
                            let mut username1 = swiper.clone();
                            let mut username2 = swipe.swiped.clone();
                            if username1 >= username2 {
                                std::mem::swap(&mut username1, &mut username2);
                            }
                            let new_match = DBMatch {
                                username1: username1.clone(),
                                username2: username2.clone(),
                            };
                            match diesel::insert_into(matches::table)
                                .values(new_match)
                                .get_result::<DBMatch>(&conn)
                            {
                                Ok(_) => println!(
                                    "saved a new match for `{}` and `{}`",
                                    username1, username2
                                ),
                                Err(err) => println!(
                                    "error creating new match for `{}` and `{}`: {:?}",
                                    username1, username2, err
                                ),
                            }
                        }
                        Err(err) => println!("pair finder error: {:?}", err),
                    }
                    HttpResponse::Ok().finish()
                }
                Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
            }
        }
        None => HttpResponse::InternalServerError().body("unable to connect to db"),
    }
}

use std::{sync::Arc, time::Duration};

use actix_web::{web, HttpRequest, HttpResponse, Responder};
use diesel::{
    dsl::{exists, not},
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
    request: HttpRequest,
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    let ext = request.extensions();
    let username = match ext.get::<DBUser>() {
        Some(u) => &u.username,
        None => panic!("route must always be accessed through auth")
    };

    let conn = match conn_pool.get_timeout(Duration::from_millis(500)) {
        Ok(conn) => conn,
        Err(err) => {
            eprintln!("failed to get pg conn for available handler");
            return HttpResponse::InternalServerError().body(err.to_string())
        },
    };

    let not_swiped_on = users::table
        .filter(
            not(exists(
                swipes::table
                    .filter(swipes::swiper.eq(&username))
                    .filter(swipes::swiped.eq(users::username)),
            ))
            .and(users::username.ne(&username)),
        )
        .filter(not(users::lat.is_null()))
        .filter(not(users::long.is_null()))
        .limit(10);

    match not_swiped_on.load::<DBUser>(&conn) {
        Ok(users) => {
            let as_string = serde_json::to_string(&users).expect("failed to jsonify DBUsers");
            HttpResponse::Ok().body(as_string)
        }
        Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
    }
}

pub async fn do_swipe(
    swipe: web::Json<SwipeDTO>,
    request: HttpRequest,
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    let ext = request.extensions();
    let swiper = match ext.get::<DBUser>() {
        Some(u) => &u.username,
        None => return HttpResponse::BadRequest().body("user not set on request"),
    };

    let conn = match conn_pool.get_timeout(Duration::from_millis(500)) {
        Ok(conn) => conn,
        Err(err) => {
            eprintln!("failed to get pg conn for do_swipe handler");
            return HttpResponse::InternalServerError().body(err.to_string())
        },
    };

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
            if swipes::table
                .filter(swipes::swiper.eq(&swipe.swiped))
                .filter(swipes::swiped.eq(swiper.clone()))
                .filter(swipes::status.eq(true))
                .first::<DBSwipe>(&conn)
                .is_ok()
            {
                let mut username1 = swiper.clone();
                let mut username2 = swipe.swiped;
                if username1 >= username2 {
                    std::mem::swap(&mut username1, &mut username2);
                }
                let new_match = DBMatch {
                    username1: username1.clone(),
                    username2: username2.clone(),
                };
                if let Err(err) = diesel::insert_into(matches::table)
                    .values(new_match)
                    .get_result::<DBMatch>(&conn)
                {
                    eprintln!(
                        "error creating new match for `{}` and `{}`: {:?}",
                        username1, username2, err
                    )
                }
            }

            HttpResponse::Ok().finish()
        }
        Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
    }
}

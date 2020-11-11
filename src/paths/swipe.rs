use std::sync::Arc;

use actix_web::{web, HttpResponse, Responder};
use diesel::{
    r2d2::{ConnectionManager, Pool},
    BoolExpressionMethods, ExpressionMethods, Insertable, PgConnection, QueryDsl, RunQueryDsl,
};
use serde::{Deserialize, Serialize};

use crate::db::{DBMatch, DBSwipe, DBUser};
use crate::schema::matches;
use crate::schema::swipes;
use crate::schema::users;
use actix_session::Session;

use serde_json;

#[derive(Insertable, Serialize, Deserialize, Debug)]
#[table_name = "swipes"]
pub struct SwipeDTO {
    swiped: String,
    status: bool,
}

pub async fn available(
    session: Session,
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    let username = session
        .get::<String>("username")
        .expect("session should be active")
        .expect("username should be got by middleware");

    match conn_pool.try_get() {
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
    session: Session,
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    let swiper = session
        .get::<String>("username")
        .expect("session should be active")
        .expect("username should be got by middleware");
    match conn_pool.try_get() {
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
                    if let Ok(_) = swipes::dsl::swipes
                        .filter(swipes::swiper.eq(&swipe.swiped))
                        .filter(swipes::swiped.eq(&swiper))
                        .filter(swipes::status.eq(true))
                        .first::<DBSwipe>(&conn)
                    {
                        let mut username1 = swiper.clone();
                        let mut username2 = swipe.swiped.clone();
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
                            println!(
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
        None => HttpResponse::InternalServerError().body("unable to connect to db"),
    }
}

#[derive(Debug, Serialize)]
struct UserMatch {
    name: String,
}

impl UserMatch {
    fn from_record(username: &str, m: DBMatch) -> UserMatch {
        let other = if m.username1 == username {
            m.username2
        } else {
            m.username1
        };

        UserMatch { name: other }
    }
}

pub async fn matches(
    session: Session,
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    let user = session
        .get::<String>("username")
        .expect("session should be active")
        .expect("username should be got by middleware");

    match conn_pool.get_ref().try_get() {
        Some(conn) => match matches::dsl::matches
            .filter(matches::username1.eq(&user))
            .or_filter(matches::username2.eq(&user))
            .load::<DBMatch>(&conn)
        {
            Ok(matches) => {
                let matches: Vec<UserMatch> = matches
                    .into_iter()
                    .map(|m| UserMatch::from_record(&user, m))
                    .collect();
                match serde_json::to_string(&matches) {
                    Ok(match_str) => HttpResponse::Ok().body(match_str),
                    Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
                }
            }
            Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
        },
        None => HttpResponse::InternalServerError().body("unable to connect to db"),
    }
}

pub async fn delete_match(
    session: Session,
    other: web::Path<String>,
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    let user = session
        .get::<String>("username")
        .expect("session should be active")
        .expect("username should be got by middleware");
    let other = other.into_inner();
    match conn_pool.try_get() {
        Some(conn) => match diesel::delete(
            matches::dsl::matches
                .filter(
                    matches::username1
                        .eq(&user)
                        .and(matches::username2.eq(&other)),
                )
                .or_filter(
                    matches::username1
                        .eq(&other)
                        .and(matches::username2.eq(&user)),
                ),
        )
        .execute(&conn)
        {
            Ok(_) => HttpResponse::Ok().finish(),
            Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
        },
        None => HttpResponse::InternalServerError().body("unable to connect to db"),
    }
}

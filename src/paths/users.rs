use std::{sync::Arc, time::Duration};

use actix_session::Session;
use actix_web::{web, HttpResponse, Responder};
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::{ExpressionMethods, PgConnection, QueryDsl, RunQueryDsl};
use serde::{Deserialize, Serialize};

use crate::db::DBUser;
use crate::schema::users;

#[derive(Serialize, Deserialize, Debug)]
pub struct UserDTO {
    username: String,
    password: String,
}

#[derive(Serialize, Deserialize)]
pub struct LatLongDTO {
    lat: f64,
    long: f64,
}

pub async fn get_users(
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    match conn_pool.get_timeout(Duration::from_millis(500)) {
        Ok(conn) => match users::table.load::<DBUser>(&conn) {
            Ok(db_users) => {
                let as_string =
                    serde_json::to_string(&db_users).expect("unable to jsonify user records");
                HttpResponse::Ok().body(as_string)
            }
            Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
        },
        Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
    }
}

pub async fn get_user(
    info: web::Path<String>,
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    let username = info.into_inner();
    match conn_pool.get_timeout(Duration::from_millis(500)) {
        Ok(conn) => match users::table.find(username).first::<DBUser>(&conn) {
            Ok(dbu) => {
                let as_string = serde_json::to_string(&dbu).expect("unable to jsonify UserDTO");
                HttpResponse::Ok().body(as_string)
            }
            Err(err) => HttpResponse::NotFound().body(err.to_string()),
        },
        Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
    }
}

pub async fn create_user(
    session: Session,
    user: web::Json<UserDTO>,
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    let mut user = user.into_inner();
    user.password = bcrypt::hash(&user.password, 10).expect("unable to encrypt user password");

    match conn_pool.get_timeout(Duration::from_millis(500)) {
        Ok(conn) => {
            let user = DBUser {
                username: user.username,
                password: user.password,
                lat: None,
                long: None,
                bio: None,
            };
            match diesel::insert_into(users::table)
                .values(&user)
                .get_result::<DBUser>(&conn)
            {
                Ok(user_record) => {
                    if let Err(err) = session.set("username", &user_record.username) {
                        println!("error setting username in session: {:?}", err);
                    }
                    let as_string =
                        serde_json::to_string(&user_record).expect("failed to jsonify DBUser");
                    HttpResponse::Ok().body(as_string)
                }
                Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
            }
        }
        Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
    }
}

pub async fn login(
    session: Session,
    user: web::Json<UserDTO>,
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    let user = user.into_inner();
    match conn_pool.get_timeout(Duration::from_millis(500)) {
        Ok(conn) => match users::table.find(&user.username).first::<DBUser>(&conn) {
            Ok(db_user) => match bcrypt::verify(&user.password, &db_user.password) {
                Ok(true) => {
                    if let Err(err) = session.set("username", db_user.username) {
                        println!("err setting username in session: {:?}", err);
                    }

                    HttpResponse::Ok().finish()
                }
                Ok(false) => HttpResponse::BadRequest().body("password incorrect"),
                Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
            },
            Err(err) => HttpResponse::BadRequest().body(err.to_string()),
        },
        Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
    }
}

pub async fn logout(session: Session) -> impl Responder {
    session.remove("username");
    HttpResponse::Ok().finish()
}

pub async fn check_login(
    session: Session,
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    match session.get::<String>("username") {
        Ok(Some(username)) => match conn_pool.get() {
            Ok(conn) => match users::table
                .filter(users::username.eq(&username))
                .first::<DBUser>(&conn)
            {
                Ok(user) => {
                    let as_string = serde_json::to_string(&user).expect("failed to jsonify DBUser");
                    HttpResponse::Ok().body(as_string)
                }
                Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
            },
            Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
        },
        _ => HttpResponse::BadRequest().body("missing username from session cookie"),
    }
}

pub async fn set_location(
    session: Session,
    latlong: web::Json<LatLongDTO>,
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    match session.get::<String>("username") {
        Ok(Some(username)) => {
            let ll = latlong.into_inner();
            match conn_pool.get_timeout(Duration::from_millis(500)) {
                Ok(conn) => {
                    match diesel::update(users::table.filter(users::username.eq(username)))
                        .set((users::lat.eq(ll.lat), users::long.eq(ll.long)))
                        .execute(&conn)
                    {
                        Ok(_) => HttpResponse::Ok().finish(),
                        Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
                    }
                }
                Err(err) => HttpResponse::Ok().body(err.to_string()),
            }
        }
        _ => HttpResponse::BadRequest().body("missing user from session cookie"),
    }
}

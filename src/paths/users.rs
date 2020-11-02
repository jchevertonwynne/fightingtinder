use std::sync::Arc;

use actix_web::{get, HttpMessage, HttpRequest, HttpResponse, post, Responder, Scope, web};
use actix_web::cookie::Cookie;
use diesel::{ExpressionMethods, Insertable, PgConnection, QueryDsl, RunQueryDsl};
use diesel::r2d2::{ConnectionManager, Pool};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use jsonwebtoken;
use serde::{Deserialize, Serialize};

use crate::db::DBUser;
use crate::schema::users;

const SECRET: &[u8] = b"some-secret";

#[derive(Insertable, Serialize, Deserialize, Debug)]
#[table_name = "users"]
struct UserDTO {
    username: String,
    password: String,
}

#[derive(Serialize, Deserialize)]
struct UserJWT {
    username: String,
    exp: usize,
}

impl From<&DBUser> for UserDTO {
    fn from(db_user: &DBUser) -> Self {
        Self {
            username: db_user.name.clone(),
            password: db_user.password.clone(),
        }
    }
}

impl From<&UserDTO> for UserJWT {
    fn from(user: &UserDTO) -> Self {
        Self {
            username: user.username.clone(),
            exp: (chrono::Utc::now() + chrono::Duration::minutes(5)).timestamp() as usize
        }
    }
}

pub fn create_paths() -> Scope {
    web::scope("/user")
        .service(create_user)
        .service(get_users)
        .service(get_user)
        .service(login)
        .service(check)
}

#[get("")]
async fn get_users(conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>) -> impl Responder {
    match conn_pool.get_ref().try_get() {
        Some(conn) => {
            match users::dsl::users.load::<DBUser>(&conn) {
                Ok(db_users) => {
                    let cleaned_users: Vec<UserDTO> = db_users.iter()
                        .map(|user| UserDTO::from(user))
                        .collect();
                    let as_string = serde_json::to_string(&cleaned_users).expect("unable to jsonify user records");
                    HttpResponse::Ok().body(as_string)
                }
                Err(err) => {
                    HttpResponse::InternalServerError().body(err.to_string())
                }
            }
        }
        None => HttpResponse::InternalServerError().body("could not get db conn instance")
    }
}

#[get("/{userid}")]
async fn get_user(req: HttpRequest, conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>) -> impl Responder {
    match conn_pool.get_ref().try_get() {
        Some(conn) => {
            let info = req.match_info();
            match info.get("userid") {
                Some(userid) => {
                    match DBUser::find(userid, &conn) {
                        Ok(dbu) => {
                            let as_string = serde_json::to_string(&UserDTO::from(&dbu)).expect("unable to jsonify UserDTO");
                            HttpResponse::Ok().body(as_string)
                        }
                        Err(err) => HttpResponse::NotFound().body(err.to_string())
                    }
                }
                None => HttpResponse::BadRequest().body("no user id provided")
            }
        }
        None => HttpResponse::InternalServerError().body("could not get db conn instance")
    }
}

#[post("/")]
async fn create_user(body: String, conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>) -> impl Responder {
    match conn_pool.get_ref().try_get() {
        Some(conn) => {
            match serde_json::from_str::<UserDTO>(&body) {
                Ok(mut user) => {
                    user.password = bcrypt::hash(user.password, 10).expect("unable to encrypt user password");
                    match diesel::insert_into(users::table).values(&user).get_result::<DBUser>(&conn) {
                        Ok(user_record) => {
                            let safe = UserDTO::from(&user_record);
                            let user_json = serde_json::to_string(&safe).expect("unable to jsonify UserDTO");

                            let token_str = jsonwebtoken::encode(&Header::default(), &UserJWT::from(&user), &EncodingKey::from_secret(SECRET)).expect("unable to encode json web token");

                            HttpResponse::Ok()
                                .cookie(Cookie::new("user", token_str))
                                .body(user_json)
                        }
                        Err(err) => HttpResponse::InternalServerError().body(err.to_string())
                    }
                }
                Err(err) => HttpResponse::BadRequest().body(err.to_string())
            }
        }
        None => HttpResponse::InternalServerError().body("could not get db conn instance")
    }
}

#[post("/login")]
async fn login(body: String, conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>) -> impl Responder {
    match conn_pool.get_ref().try_get() {
        Some(conn) => {
            match serde_json::from_str::<UserDTO>(&body) {
                Ok(user) => {
                    match users::dsl::users.filter(users::dsl::username.eq(&user.username)).limit(1).load::<DBUser>(&conn) {
                        Ok(mut db_users) => {
                            match db_users.pop() {
                                Some(db_user) => {
                                    match bcrypt::verify(&user.password, &db_user.password) {
                                        Ok(res) => {
                                            if res {
                                                let token_str = jsonwebtoken::encode(&Header::default(), &UserJWT::from(&user), &EncodingKey::from_secret(SECRET)).expect("unable to encode json web token");

                                                HttpResponse::Ok()
                                                    .cookie(Cookie::new("user", token_str))
                                                    .body(format!("{}", db_user.id))
                                            } else {
                                                HttpResponse::BadRequest().body("incorrect password")
                                            }
                                        }
                                        Err(err) => HttpResponse::InternalServerError().body(err.to_string())
                                    }
                                }
                                None => HttpResponse::BadRequest().body("user not found")
                            }
                        }
                        Err(err) =>  HttpResponse::BadRequest().body(err.to_string())
                    }
                }
                Err(err) => HttpResponse::BadRequest().body(err.to_string())
            }
        }
        None => HttpResponse::InternalServerError().body("could not get db conn instance")
    }
}

#[get("/li/")]
async fn check(req: HttpRequest) -> impl Responder {
    match req.cookie("user") {
        Some(cookie) => {
            match jsonwebtoken::decode::<UserJWT>(&cookie.value(), &DecodingKey::from_secret(SECRET), &Validation::default()) {
                Ok(decoded) => HttpResponse::Ok().body(decoded.claims.username),
                Err(err) => HttpResponse::BadRequest().body(err.to_string())
            }
        }
        None => HttpResponse::BadRequest().body("missing user cookie")
    }
}
use std::sync::Arc;

use actix_web::cookie::Cookie;
use actix_web::{get, post, web, HttpMessage, HttpRequest, HttpResponse, Responder, Scope};
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::{ExpressionMethods, Insertable, PgConnection, QueryDsl, RunQueryDsl};
use jsonwebtoken;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use time::{self, Duration};

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
            exp: (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp() as usize,
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
async fn get_users(
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    match conn_pool.get_ref().try_get() {
        Some(conn) => match users::dsl::users.load::<DBUser>(&conn) {
            Ok(db_users) => {
                let as_string =
                    serde_json::to_string(&db_users).expect("unable to jsonify user records");
                HttpResponse::Ok().body(as_string)
            }
            Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
        },
        None => HttpResponse::InternalServerError().body("could not get db conn instance"),
    }
}

#[get("/u/{userid}")]
async fn get_user(
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
    info: web::Path<i32>,
) -> impl Responder {
    let id = info.into_inner();
    match conn_pool.get_ref().try_get() {
        Some(conn) => match DBUser::find(id, &conn) {
            Ok(dbu) => {
                let as_string = serde_json::to_string(&dbu).expect("unable to jsonify UserDTO");
                HttpResponse::Ok().body(as_string)
            }
            Err(err) => HttpResponse::NotFound().body(err),
        },
        None => HttpResponse::InternalServerError().body("could not get db conn instance"),
    }
}

#[post("/")]
async fn create_user(
    body: String,
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    match conn_pool.get_ref().try_get() {
        Some(conn) => match serde_json::from_str::<UserDTO>(&body) {
            Ok(mut user) => {
                user.password =
                    bcrypt::hash(user.password, 10).expect("unable to encrypt user password");
                match diesel::insert_into(users::table)
                    .values(&user)
                    .get_result::<DBUser>(&conn)
                {
                    Ok(user_record) => {
                        let user_json =
                            serde_json::to_string(&user_record).expect("unable to jsonify UserDTO");

                        let header = Header::default();
                        let claims = UserJWT::from(&user);
                        let key = EncodingKey::from_secret(SECRET);
                        let token_str = jsonwebtoken::encode(&header, &claims, &key)
                            .expect("unable to encode json web token");

                        HttpResponse::Ok()
                            .cookie(Cookie::new("user", token_str))
                            .body(user_json)
                    }
                    Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
                }
            }
            Err(err) => HttpResponse::BadRequest().body(err.to_string()),
        },
        None => HttpResponse::InternalServerError().body("could not get db conn instance"),
    }
}

#[post("/login")]
async fn login(
    body: String,
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    match conn_pool.get_ref().try_get() {
        Some(conn) => match serde_json::from_str::<UserDTO>(&body) {
            Ok(user) => match users::dsl::users
                .filter(users::dsl::username.eq(&user.username))
                .limit(1)
                .get_result::<DBUser>(&conn)
            {
                Ok(db_user) => {
                    if let Ok(true) = bcrypt::verify(&user.password, &db_user.password) {
                        let header = Header::default();
                        let claims = UserJWT::from(&user);
                        let key = EncodingKey::from_secret(SECRET);
                        let token_str = jsonwebtoken::encode(&header, &claims, &key)
                            .expect("unable to encode json web token");

                        HttpResponse::Ok()
                            .set_header("Access-Control-Allow-Credentials", "true")
                            .cookie(Cookie::new("user", token_str))
                            .body(db_user.id.to_string())
                    } else {
                        HttpResponse::BadRequest().body("incorrect password")
                    }
                }
                Err(err) => HttpResponse::BadRequest().body(err.to_string()),
            },
            Err(err) => HttpResponse::BadRequest().body(err.to_string()),
        },
        None => HttpResponse::InternalServerError().body("could not get db conn instance"),
    }
}

#[get("/logout")]
async fn logout() -> impl Responder {
    let mut cookie = Cookie::new("user", "");
    cookie.set_max_age(Duration::seconds(0));
    HttpResponse::Ok().cookie(cookie).finish()
}

#[get("/li")]
async fn check(req: HttpRequest) -> impl Responder {
    match req.cookie("user") {
        Some(cookie) => match jsonwebtoken::decode::<UserJWT>(
            &cookie.value(),
            &DecodingKey::from_secret(SECRET),
            &Validation::default(),
        ) {
            Ok(decoded) => HttpResponse::Ok().set_header("Access-Control-Allow-Credentials", "true").body(decoded.claims.username),
            Err(err) => HttpResponse::BadRequest().set_header("Access-Control-Allow-Credentials", "true").body(err.to_string()),
        },
        None => HttpResponse::BadRequest().set_header("Access-Control-Allow-Credentials", "true").body("missing user cookie"),
    }
}

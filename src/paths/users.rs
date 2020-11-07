use std::sync::Arc;

use actix_web::{cookie::Cookie};
use actix_web::{get, post, web, HttpMessage, HttpRequest, HttpResponse, Responder, Scope};
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::{ExpressionMethods, Insertable, PgConnection, QueryDsl, RunQueryDsl};
use jsonwebtoken::{self, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use time::Duration;

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
            username: db_user.username.clone(),
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
        .service(logout)
        .service(check)
        .service(set_location)
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

#[get("/u/{username}")]
async fn get_user(
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
    info: web::Path<String>,
) -> impl Responder {
    let id = info.into_inner();
    match conn_pool.get_ref().try_get() {
        Some(conn) => match users::dsl::users.find(id).first::<DBUser>(&conn) {
            Ok(dbu) => {
                let as_string = serde_json::to_string(&dbu).expect("unable to jsonify UserDTO");
                HttpResponse::Ok().body(as_string)
            }
            Err(err) => HttpResponse::NotFound().body(err.to_string()),
        },
        None => HttpResponse::InternalServerError().body("could not get db conn instance"),
    }
}

#[post("")]
async fn create_user(
    body: String,
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    match serde_json::from_str::<UserDTO>(&body) {
        Ok(mut user) => {
            user.password =
                bcrypt::hash(user.password, 10).expect("unable to encrypt user password");

            match conn_pool.get_ref().try_get() {
                Some(conn) => {
                    match diesel::insert_into(users::table)
                        .values(&user)
                        .get_result::<DBUser>(&conn)
                    {
                        Ok(user_record) => {
                            let user_json = serde_json::to_string(&user_record)
                                .expect("unable to jsonify UserDTO");

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
                None => HttpResponse::InternalServerError().body("could not get db conn instance"),
            }
        }
        Err(err) => HttpResponse::BadRequest().body(err.to_string()),
    }
}

#[post("/login")]
async fn login(
    body: String,
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    match serde_json::from_str::<UserDTO>(&body) {
        Ok(user) => match conn_pool.get_ref().try_get() {
            Some(conn) => match users::dsl::users
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
                            .cookie(Cookie::new("user", token_str))
                            .finish()
                    } else {
                        HttpResponse::BadRequest().body("incorrect password")
                    }
                }
                Err(err) => HttpResponse::BadRequest().body(err.to_string()),
            },
            None => HttpResponse::InternalServerError().body("could not get db conn instance"),
        },
        Err(err) => HttpResponse::BadRequest().body(err.to_string()),
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
            Ok(decoded) => HttpResponse::Ok()
                .body(decoded.claims.username),
            Err(err) => HttpResponse::BadRequest()
                .body(err.to_string()),
        },
        None => HttpResponse::BadRequest()
            .body("missing user cookie"),
    }
}

#[derive(Serialize, Deserialize)]
struct LatLong {
    lat: f64,
    long: f64,
}

#[post("/location")]
async fn set_location(
    body: String,
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    match serde_json::from_str::<LatLong>(&body) {
        Ok(ll) => match conn_pool.as_ref().try_get() {
            Some(conn) => {
                match diesel::update(users::dsl::users.filter(users::dsl::username.eq("joe")))
                    .set((users::dsl::lat.eq(ll.lat), users::dsl::long.eq(ll.long)))
                    .execute(&conn)
                {
                    Ok(_) => HttpResponse::Ok().finish(),
                    Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
                }
            }
            None => HttpResponse::Ok().finish(),
        },
        Err(err) => HttpResponse::BadRequest().body(err.to_string()),
    }
}

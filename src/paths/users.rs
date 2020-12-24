use std::{fs, io::Write, sync::Arc, time::Duration};

use actix_multipart::Multipart;
use actix_session::Session;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::{ExpressionMethods, PgConnection, QueryDsl, RunQueryDsl};
use futures_util::{StreamExt, TryStreamExt};
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

#[derive(Serialize, Deserialize)]
pub struct BioDTO {
    bio: String,
}

pub async fn get_users(
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    let conn = match conn_pool.get_timeout(Duration::from_millis(500)) {
        Ok(conn) => conn,
        Err(err) => return HttpResponse::InternalServerError().body(err.to_string()),
    };

    let db_users = match users::table.load::<DBUser>(&conn) {
        Ok(db_users) => db_users,
        Err(err) => return HttpResponse::InternalServerError().body(err.to_string()),
    };

    let as_string = serde_json::to_string(&db_users).expect("unable to jsonify user records");
    HttpResponse::Ok().body(as_string)
}

pub async fn get_user_pic(
    info: web::Path<String>,
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    let username = info.into_inner();

    let conn = match conn_pool.get_timeout(Duration::from_millis(500)) {
        Ok(conn) => conn,
        Err(err) => return HttpResponse::InternalServerError().body(err.to_string()),
    };

    let dbu: DBUser = match users::table.find(username).first::<DBUser>(&conn) {
        Ok(dbu) => dbu,
        Err(err) => return HttpResponse::NotFound().body(err.to_string()),
    };

    let filename = match &dbu.profile_pic {
        Some(s) => s.as_str(),
        None => return HttpResponse::NotFound().finish(),
    };

    let contents = match fs::read(filename) {
        Ok(c) => c,
        Err(err) => return HttpResponse::InternalServerError().body(err.to_string()),
    };

    HttpResponse::Ok().body(contents)
}

pub async fn create_user(
    session: Session,
    user: web::Json<UserDTO>,
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    let mut user = user.into_inner();
    user.password = bcrypt::hash(&user.password, 10).expect("unable to encrypt user password");

    let conn = match conn_pool.get_timeout(Duration::from_millis(500)) {
        Ok(conn) => conn,
        Err(err) => return HttpResponse::InternalServerError().body(err.to_string()),
    };

    let user = DBUser {
        username: user.username,
        password: user.password,
        lat: None,
        long: None,
        bio: None,
        profile_pic: None,
    };
    let user_record = match diesel::insert_into(users::table)
        .values(&user)
        .get_result::<DBUser>(&conn)
    {
        Ok(user_record) => user_record,
        Err(err) => return HttpResponse::InternalServerError().body(err.to_string()),
    };

    if let Err(err) = session.set("username", &user_record.username) {
        eprintln!("error setting username in session: {:?}", err);
    }

    let as_string = serde_json::to_string(&user_record).expect("failed to jsonify DBUser");
    HttpResponse::Ok().body(as_string)
}

pub async fn login(
    session: Session,
    user: web::Json<UserDTO>,
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    let user = user.into_inner();

    let conn = match conn_pool.get_timeout(Duration::from_millis(500)) {
        Ok(conn) => conn,
        Err(err) => return HttpResponse::InternalServerError().body(err.to_string()),
    };

    let db_user = match users::table.find(&user.username).first::<DBUser>(&conn) {
        Ok(db_user) => db_user,
        Err(err) => return HttpResponse::BadRequest().body(err.to_string()),
    };

    let found = match bcrypt::verify(&user.password, &db_user.password) {
        Ok(found) => found,
        Err(err) => return HttpResponse::InternalServerError().body(err.to_string()),
    };

    if found {
        if let Err(err) = session.set("username", db_user.username) {
            eprintln!("err setting username in session: {:?}", err);
        }
        HttpResponse::Ok().finish()
    } else {
        HttpResponse::BadRequest().body("password incorrect")
    }
}

pub async fn logout(session: Session) -> impl Responder {
    session.remove("username");
    HttpResponse::Ok().finish()
}

pub async fn check_login(
    request: HttpRequest,
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    let ext = request.extensions();
    let username = match ext.get::<DBUser>() {
        Some(u) => u.username.as_str(),
        None => return HttpResponse::BadRequest().body("user not set on request"),
    };

    let conn = match conn_pool.get_timeout(Duration::from_millis(500)) {
        Ok(conn) => conn,
        Err(err) => return HttpResponse::InternalServerError().body(err.to_string()),
    };

    let user = match users::table
        .filter(users::username.eq(&username))
        .first::<DBUser>(&conn)
    {
        Ok(user) => user,
        Err(err) => return HttpResponse::InternalServerError().body(err.to_string()),
    };

    let as_string = serde_json::to_string(&user).expect("failed to jsonify DBUser");
    HttpResponse::Ok().body(as_string)
}

pub async fn set_location(
    request: HttpRequest,
    latlong: web::Json<LatLongDTO>,
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    let ext = request.extensions();
    let username = match ext.get::<DBUser>() {
        Some(u) => u.username.as_str(),
        None => return HttpResponse::BadRequest().body("user not set on request"),
    };

    let ll = latlong.into_inner();

    let conn = match conn_pool.get_timeout(Duration::from_millis(500)) {
        Ok(conn) => conn,
        Err(err) => return HttpResponse::InternalServerError().body(err.to_string()),
    };

    match diesel::update(users::table.filter(users::username.eq(username)))
        .set((users::lat.eq(ll.lat), users::long.eq(ll.long)))
        .execute(&conn)
    {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
    }
}

pub async fn set_bio(
    request: HttpRequest,
    latlong: web::Json<BioDTO>,
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    let ext = request.extensions();
    let username = match ext.get::<DBUser>() {
        Some(user) => user.username.as_str(),
        None => return HttpResponse::BadRequest().body("user not set on request"),
    };

    let bio = latlong.into_inner();

    let conn = match conn_pool.get_timeout(Duration::from_millis(500)) {
        Ok(conn) => conn,
        Err(err) => return HttpResponse::InternalServerError().body(err.to_string()),
    };

    match diesel::update(users::table.filter(users::username.eq(username)))
        .set(users::bio.eq(bio.bio))
        .execute(&conn)
    {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
    }
}

pub async fn upload_profile_pic(
    request: HttpRequest,
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
    mut payload: Multipart,
) -> impl Responder {
    let ext = request.extensions();
    let username = match ext.get::<DBUser>() {
        Some(user) => user.username.as_str(),
        None => return HttpResponse::BadRequest().body("user not set on request"),
    };

    let mut field = match payload.try_next().await {
        Ok(Some(field)) => field,
        _ => return HttpResponse::BadRequest().body("missing file upload"),
    };

    let content_type = match field.content_disposition() {
        Some(disposition) => disposition,
        None => return HttpResponse::BadRequest().body("bad file upload lol"),
    };

    let raw_filename = match content_type.get_filename() {
        Some(filename) => filename,
        None => return HttpResponse::BadRequest().body("unable to determine filename"),
    };

    let file_type = match raw_filename.find('.').map(|i| &raw_filename[i + 1..]) {
        Some(f_type) => f_type,
        None => return HttpResponse::BadRequest().body("bad filename"),
    };

    let filename = format!("./tmp/{}.{}", username, file_type);
    let filename_to_make = filename.clone();

    let mut f = web::block(|| fs::File::create(filename_to_make))
        .await
        .unwrap();

    while let Some(chunk) = field.next().await {
        let data = chunk.unwrap();
        // filesystem operations are blocking, we have to use threadpool
        f = web::block(move || f.write_all(&data).map(|_| f))
            .await
            .expect("pls");
    }

    let conn = match conn_pool.get_timeout(Duration::from_millis(500)) {
        Ok(conn) => conn,
        Err(err) => return HttpResponse::InternalServerError().body(err.to_string()),
    };

    match diesel::update(users::table.filter(users::username.eq(username)))
        .set(users::profile_pic.eq(&filename))
        .execute(&conn)
    {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
    }
}

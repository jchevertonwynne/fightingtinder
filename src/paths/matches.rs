use crate::db::{DBMatch, DBUser};
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::{BoolExpressionMethods, ExpressionMethods, PgConnection, QueryDsl, RunQueryDsl};
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;

use crate::schema::matches;

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
    request: HttpRequest,
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    let ext = request.extensions();
    let username: &str = match ext.get::<DBUser>() {
        Some(u) => &u.username,
        None => return HttpResponse::BadRequest().body("user not set on request"),
    };

    let conn = match conn_pool.get_timeout(Duration::from_millis(500)) {
        Ok(conn) => conn,
        Err(err) => return HttpResponse::InternalServerError().body(err.to_string()),
    };

    let matches = match matches::table
        .filter(matches::username1.eq(&username))
        .or_filter(matches::username2.eq(&username))
        .load::<DBMatch>(&conn)
    {
        Ok(matches) => matches,
        Err(err) => return HttpResponse::InternalServerError().body(err.to_string()),
    };

    let matches: Vec<UserMatch> = matches
        .into_iter()
        .map(|m| UserMatch::from_record(&username, m))
        .collect();
    let as_string = serde_json::to_string(&matches).expect("unable to jsonify DBUsers");
    HttpResponse::Ok().body(as_string)
}

pub async fn delete_match(
    request: HttpRequest,
    other: web::Path<String>,
    conn_pool: web::Data<Arc<Pool<ConnectionManager<PgConnection>>>>,
) -> impl Responder {
    let ext = request.extensions();
    let username: &str = match ext.get::<DBUser>() {
        Some(u) => &u.username,
        None => return HttpResponse::BadRequest().body("user not set on request"),
    };

    let conn = match conn_pool.get_timeout(Duration::from_millis(500)) {
        Ok(conn) => conn,
        Err(err) => return HttpResponse::InternalServerError().body(err.to_string()),
    };

    let other = other.into_inner();
    match diesel::delete(
        matches::table
            .filter(
                matches::username1
                    .eq(&username)
                    .and(matches::username2.eq(&other)),
            )
            .or_filter(
                matches::username1
                    .eq(&other)
                    .and(matches::username2.eq(&username)),
            ),
    )
    .execute(&conn)
    {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(err) => HttpResponse::InternalServerError().body(err.to_string()),
    }
}

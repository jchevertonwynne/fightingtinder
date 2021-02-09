use std::sync::Arc;

use actix_session::CookieSession;
use actix_web::{web, App, HttpServer};
use diesel::r2d2::{ConnectionManager, Pool};

use web::{get, post, scope};

use diesel::PgConnection;
use fightingtinder::auth::SessionChecker;
use fightingtinder::paths::{matches, swipe, users};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv::dotenv().ok();
    let session_secret = dotenv::var("SESSION_SECRET").expect("SESSION_SECRET should be set");
    let database_url = dotenv::var("DATABASE_URL").expect("DATABASE_URL env var should be set");
    let manager = ConnectionManager::<PgConnection>::new(&database_url);
    let pg_pool = Arc::new(
        Pool::builder()
            .max_size(200)
            .build(manager)
            .expect("unable to create pool of pg connections"),
    );

    println!("created pg pool");

    let rd_pool = r2d2_redis::RedisConnectionManager::new("redis://127.0.0.1").expect("unable to create connection manager");
    let rd_pool = Arc::new(r2d2_redis::r2d2::Pool::builder()
        .max_size(200)
        .build(rd_pool)
        .expect("unable to create redis pool"));

    println!("created rd pool");

    HttpServer::new(move || {
        App::new()
            .wrap(CookieSession::signed(session_secret.as_bytes()).secure(false))
            .data(Arc::clone(&pg_pool))
            .data(Arc::clone(&rd_pool))
            .service(
                scope("/user")
                    .route("", get().to(users::get_users))
                    .route("", post().to(users::create_user))
                    .route("/u/{username}", get().to(users::get_user_pic))
                    .route("/login", post().to(users::login))
                    .route("/logout", get().to(users::logout))
                    .service(
                        scope("/manage")
                            .wrap(SessionChecker::new(Arc::clone(&pg_pool)))
                            .route("/li", get().to(users::check_login))
                            .route("/location", post().to(users::set_location))
                            .route("/bio", post().to(users::set_bio))
                            .route("/profile_pic", post().to(users::upload_profile_pic)),
                    ),
            )
            .service(
                scope("/swipe")
                    .wrap(SessionChecker::new(Arc::clone(&pg_pool)))
                    .route("", post().to(swipe::do_swipe))
                    .route("/available", get().to(swipe::available)),
            )
            .service(
                scope("/match")
                    .wrap(SessionChecker::new(Arc::clone(&pg_pool)))
                    .route("/", get().to(matches::matches))
                    .route("/{username}", web::delete().to(matches::delete_match)),
            )
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}

use std::sync::Arc;

use actix_session::CookieSession;
use actix_web::{web, App, HttpServer};
use web::{get, post, scope};

use fightingtinder::auth::SessionChecker;
use fightingtinder::db::connection_pool;
use fightingtinder::paths::{swipe, users};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv::dotenv().ok();
    let session_secret = dotenv::var("SESSION_SECRET").expect("SESSION_SECRET should be set");

    HttpServer::new(move || {
        let pool = connection_pool().expect("unable to create pool of connections");
        let pool = Arc::new(pool);

        App::new()
            .wrap(CookieSession::signed(session_secret.as_bytes()).secure(false))
            .data(Arc::clone(&pool))
            .service(
                scope("/user")
                    .route("", get().to(users::get_users))
                    .route("", post().to(users::create_user))
                    .route("/u/{username}", get().to(users::get_user))
                    .route("/login", post().to(users::login))
                    .route("/logout", get().to(users::logout))
                    .service(
                        scope("/manage")
                            .wrap(SessionChecker::new(Arc::clone(&pool)))
                            .route("/li", get().to(users::check_login))
                            .route("/location", post().to(users::set_location)),
                    ),
            )
            .service(
                scope("/swipe")
                    .wrap(SessionChecker::new(Arc::clone(&pool)))
                    .route("", post().to(swipe::do_swipe))
                    .route("/available", get().to(swipe::available))
                    .route("/matches", get().to(swipe::matches))
                    .route("/match/{username}", web::delete().to(swipe::delete_match)),
            )
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}

use actix_web::{App, HttpServer};

use fightingtinder::{db::connection_pool, paths};
use std::sync::Arc;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(move || {
        let pool = connection_pool().expect("please make a pool");
        let pool = Arc::new(pool);
        App::new()
        .data(pool)
        .service( paths::users::create_paths())
    }).bind("127.0.0.1:8080")?
        .run()
        .await
}
use std::{sync::Arc, time::Duration};

use actix_session::UserSession;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::{Error, HttpMessage, HttpResponse};
use diesel::{
    r2d2::{ConnectionManager, Pool},
    PgConnection, QueryDsl, RunQueryDsl,
};
use futures_util::future::{Either, Ready};
use futures_util::task::{Context, Poll};

use crate::{db::DBUser, schema::users};
use futures_util::future;

pub struct SessionChecker {
    conn_pool: Arc<Pool<ConnectionManager<PgConnection>>>,
}

impl SessionChecker {
    pub fn new(conn_pool: Arc<Pool<ConnectionManager<PgConnection>>>) -> Self {
        SessionChecker { conn_pool }
    }
}

impl<S, B> Transform<S> for SessionChecker
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = S::Error;
    type Transform = SessionCheckerMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        future::ok(SessionCheckerMiddleware {
            service,
            conn_pool: Arc::clone(&self.conn_pool),
        })
    }
}

pub struct SessionCheckerMiddleware<S> {
    service: S,
    conn_pool: Arc<Pool<ConnectionManager<PgConnection>>>,
}

impl<S, B> Service for SessionCheckerMiddleware<S>
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Either<S::Future, Ready<Result<Self::Response, Self::Error>>>;

    fn poll_ready(&mut self, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&mut self, req: ServiceRequest) -> Self::Future {
        let session = req.get_session();
        let username = match session
            .get::<String>("username")
            .expect("method literally cannot fail")
        {
            Some(username) => username,
            None => {
                return Either::Right(future::ok(req.into_response(
                    HttpResponse::BadRequest()
                        .body("missing username from session cookie")
                        .into_body(),
                )))
            }
        };

        let conn = match self.conn_pool.get_timeout(Duration::from_millis(500)) {
            Ok(c) => c,
            Err(err) => {
                return Either::Right(future::ok(req.into_response(
                    HttpResponse::InternalServerError()
                        .body(err.to_string())
                        .into_body(),
                )))
            }
        };

        match users::table.find(username).first::<DBUser>(&conn) {
            Ok(user) => {
                req.extensions_mut().insert(user);
                Either::Left(self.service.call(req))
            }
            Err(err) => {
                session.remove("username");
                Either::Right(future::ok(req.into_response(
                    HttpResponse::BadRequest().body(err.to_string()).into_body(),
                )))
            }
        }
    }
}

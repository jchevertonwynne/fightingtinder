use std::sync::Arc;

use actix_session::UserSession;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::{Error, HttpResponse};
use diesel::{
    r2d2::{ConnectionManager, Pool},
    PgConnection, QueryDsl, RunQueryDsl,
};
use futures_util::future::{ok, Either, Ready};
use futures_util::task::{Context, Poll};

use crate::{db::DBUser, schema::users};

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
        ok(SessionCheckerMiddleware {
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
        match session
            .get::<String>("username")
            .expect("method literally cannot fail")
        {
            Some(username) => match self.conn_pool.try_get() {
                Some(conn) => match users::dsl::users.find(username).first::<DBUser>(&conn) {
                    Ok(_) => Either::Left(self.service.call(req)),
                    Err(err) => {
                        session.remove("username");
                        Either::Right(ok(req.into_response(
                            HttpResponse::BadRequest().body(err.to_string()).into_body(),
                        )))
                    }
                },
                None => Either::Right(ok(req.into_response(
                    HttpResponse::InternalServerError()
                        .body("could not get db connection")
                        .into_body(),
                ))),
            },
            None => Either::Right(ok(req.into_response(
                HttpResponse::BadRequest()
                    .body("missing username from session cookie")
                    .into_body(),
            ))),
        }
    }
}

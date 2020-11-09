use actix_session::UserSession;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::{Error, HttpMessage, HttpResponse};
use futures_util::future::{ok, Either, Ready};
use futures_util::task::{Context, Poll};

pub struct SessionChecker {}

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
        ok(SessionCheckerMiddleware { service })
    }
}

pub struct SessionCheckerMiddleware<S> {
    service: S,
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
        match session.get::<String>("username") {
            Ok(u_opt) => match u_opt {
                Some(username) => {
                    req.extensions_mut().insert(username);
                    Either::Left(self.service.call(req))
                }
                None => Either::Right(ok(req.into_response(
                    HttpResponse::BadRequest()
                        .body("missing username from session cookie")
                        .into_body(),
                ))),
            },
            Err(err) => Either::Right(ok(req.into_response(
                HttpResponse::InternalServerError()
                    .body(err.to_string())
                    .into_body(),
            ))),
        }
    }
}

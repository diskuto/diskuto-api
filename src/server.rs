use std::{fmt, net::TcpListener};

use actix_web::http::header::HeaderValue;
use backend::FactoryBox;
use futures::Future;

use actix_web::{middleware::DefaultHeaders, HttpResponse, body};
use actix_web::http::{Method, header};

use actix_web::web::{
    self,
    get,
    put,
    route,
    Data,
};
use actix_web::dev::{Service, ServiceRequest, ServiceResponse};

use actix_web::{App, HttpServer};
use anyhow::Context;


use crate::ServeCommand;
use crate::backend;

mod attachments;
mod html;
mod pagination;
mod rest;
mod non_standard;


pub(crate) fn serve(command: ServeCommand) -> Result<(), anyhow::Error> {

    env_logger::init_from_env(env_logger::Env::default()
        .default_filter_or("info,actix_server::worker=warn")
    );
    
    sodiumoxide::init().expect("sodiumoxide::init()");

    let ServeCommand{open, backend_options, mut binds} = command;

    let factory_box = FactoryBox{
        factory: backend_options.factory_builder()?.factory()?
    };

    let app_factory = move || {
        let data = Data::new(
            AppData{
                backend_factory: factory_box.factory.dyn_clone(),
            }
        );
        let mut app = App::new()
            .wrap(actix_web::middleware::Logger::default())
            .app_data(data)
            ;
        app = app.configure(api_routes);
        
        // Soon to be deprecated.  (First: upgrade mastodon & RSS scripts)
        app = app.configure(deprecated_api_routes);
        app = app.configure(html::routes);

        return app;
    };

    if binds.is_empty() {
        binds.push("127.0.0.1:8080".into());
    }

    let mut server = HttpServer::new(app_factory); 
    
    for bind in &binds {
        let socket = open_socket(bind).with_context(|| {
            format!("Error binding to address/port: {}", bind)
        })?;
        server = server.listen(socket)?;
    }

    if open {
        // TODO: This opens up a (AFAICT) blocking CLI browser on Linux. Boo. Don't do that.
        // TODO: Handle wildcard addresses (0.0.0.0, ::0) and --open them via localhost.
        let url = format!("http://{}/", binds[0]);
        let opened = webbrowser::open(&url);
        if !opened.is_ok() {
            println!("Warning: Couldn't open browser.");
        }
    }

    for bind in &binds {
        println!("Started at: http://{}/", bind);
    }
 
    let system = actix_web::rt::System::new();
    system.block_on(server.run())?;
   
    Ok(())
}

// Work around https://github.com/actix/actix-web/issues/1913
fn open_socket(bind: &str) -> Result<TcpListener, anyhow::Error> {
    use socket2::{Domain, Protocol, Socket, Type};
    use std::net::SocketAddr;
    
    // Eh, this is what actix was using:
    let backlog = 1024;
    
    let addr = bind.parse()?;
    let domain = match addr {
        SocketAddr::V4(_) => Domain::IPV4,
        SocketAddr::V6(_) => Domain::IPV6,
    };
    let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))?;
    socket.bind(&addr.into())?;
    socket.listen(backlog)?;

    Ok(socket.into())
}


/// Data available for our whole application.
/// Gets stored in a Data<AppData>
// This is so that we have typesafe access to AppData fields, because actix
// Data<Foo> can fail at runtime if you delete a Foo and don't clean up after
// yourself.
pub(crate) struct AppData {
    backend_factory: Box<dyn backend::Factory>,
}

fn api_routes(cfg: &mut web::ServiceConfig) {
    cfg
        .service(
            web::resource("/diskuto/homepage")
            .route(get().to(rest::homepage_item_list))
            .wrap(cors_ok_headers())
        )

        .service(
            web::resource("/diskuto/users/{user_id}/profile")
            .route(get().to(rest::get_profile_item))
            .wrap(cors_ok_headers())
        )
        .service(
            web::resource("/diskuto/users/{user_id}/items")
            .route(get().to(rest::user_item_list))
            .wrap(cors_ok_headers())
        )
        .service(
            web::resource("/diskuto/users/{user_id}/feed")
            .route(get().to(rest::feed_item_list))
            .wrap(cors_ok_headers())
        )

        // Not really part of the standard, but useful to have:
        .service(
            web::resource("/diskuto/users/{user_id}/icon.png")
            .route(get().to(non_standard::identicon_get))
            .wrap_fn(immutable_etag)
        )
        .service(
            web::resource("/diskuto/users/{userID}/items/{signature}")
            .route(get().to(rest::get_item))
            .route(put().to(rest::put_item))
            .route(route().method(Method::OPTIONS).to(cors_preflight_allow))
            .wrap(cors_ok_headers())
            .wrap_fn(immutable_etag)
        )
        .service(
            web::resource("/diskuto/users/{user_id}/items/{signature}/replies")
            .route(get().to(rest::item_reply_list))
            .wrap(cors_ok_headers())
        ).service(
            web::resource("/diskuto/users/{user_id}/items/{signature}/files/{file_name}")
            .route(get().to(attachments::get_file))
            .route(put().to(attachments::put_file))
            .route(route().method(Method::HEAD).to(attachments::head_file))
            .route(route().method(Method::OPTIONS).to(cors_preflight_allow))
            .wrap(cors_ok_headers())
            .wrap_fn(immutable_etag)
        )
    ;
}

// These were the old URL layout with FeoBlog, before Diskuto.
fn deprecated_api_routes(cfg: &mut web::ServiceConfig) {
    // TODO: Add some warning that these old routes are being accessed.
    cfg
        .service(
            web::resource("/homepage/proto3")
            .route(get().to(rest::homepage_item_list))
            .wrap(cors_ok_headers())
        )
        .service(
            web::resource("/u/{user_id}/proto3")
            .route(get().to(rest::user_item_list))
            .wrap(cors_ok_headers())
        )
        .service(
            web::resource("/u/{user_id}/icon.png")
            .route(get().to(non_standard::identicon_get))
            .wrap_fn(immutable_etag)
        )
        .service(
            web::resource("/u/{userID}/i/{signature}/proto3")
            .route(get().to(rest::get_item))
            .route(put().to(rest::put_item))
            .route(route().method(Method::OPTIONS).to(cors_preflight_allow))
            .wrap(cors_ok_headers())
            .wrap_fn(immutable_etag)
        )
        .service(
            web::resource("/u/{user_id}/i/{signature}/replies/proto3")
            .route(get().to(rest::item_reply_list))
            .wrap(cors_ok_headers())
        ).service(
            web::resource("/u/{user_id}/i/{signature}/files/{file_name}")
            .route(get().to(attachments::get_file))
            .route(put().to(attachments::put_file))
            .route(route().method(Method::HEAD).to(attachments::head_file))
            .route(route().method(Method::OPTIONS).to(cors_preflight_allow))
            .wrap(cors_ok_headers())
            .wrap_fn(immutable_etag)
        )
        .service(
            web::resource("/u/{user_id}/profile/proto3")
            .route(get().to(rest::get_profile_item))
            .wrap(cors_ok_headers())
        )
        .route("/u/{user_id}/feed/proto3", get().to(rest::feed_item_list))
    ;
}




fn http_not_modified() -> HttpResponse {
    // Must use a Body::None here instead of an empty body.
    //
    // See: the "Compatibility Notes" section at:
    // https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/304
    //
    // In particular, when using this behind an Apache ProxyPass config, which uses persistent
    // connections, Apache seems to always be sending an HTTP 200 with (and maybe because-of?) the
    // Content-Length == 0, instead of a 304 without a Content-Length header.
    HttpResponse::NotModified().body(body::None::new())
}

/// Browsers like to re-validate things even when they don't need to. (Say, when the user hits reload.)
/// For our content-addressable URLs, make a shortcut etag to spare us some bandwidth & DB hits:
fn immutable_etag<'a, S>(req: ServiceRequest, service: &'a S) 
-> impl Future<Output = Result<ServiceResponse, S::Error>>
where S: Service<ServiceRequest, Response=ServiceResponse>
{
    use actix_web::Either;

    let is_get = req.method() == &Method::GET;
    // If the client sends us an if-none-match, they're just sending back our "immutable" ETag.
    // This means they already have our data and are just trying to re-load it unnecessarily.
    let cache_validation_request = req.headers().get("if-none-match").is_some();


    let fut = if !cache_validation_request {
        Either::Left(service.call(req))
    } else {
        // Skip dispatching to the underlying service, and pass along the req:
        Either::Right(req)
    };
    async move {
        let res = match fut {
            Either::Left(fut) => fut.await,
            Either::Right(req) => {
                let res = req.into_response(http_not_modified());
                return Ok(res);
            }
        };

        let mut res = match res {
            // If result was an error, no caching:
            Err(r) => { return Err(r); }
            Ok(r) => r,
        };

        if is_get && res.response().status().is_success() {
            let headers = res.headers_mut();
            headers.insert(header::ETAG, HeaderValue::from_static("\"immutable\""));
                    
            // "aggressive caching" according to https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Cache-Control
            // 31536000 = 365 days, as seconds
            headers.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=31536000, no-transform, immutable")
            );
        }

        Ok(res)
    }
}


// // CORS headers must be present for *all* responses, including 404, 500, etc.
// // Applying it to each case individiaully may be error-prone, so here's a filter to do so for us.
fn cors_ok_headers() -> DefaultHeaders {
    DefaultHeaders::new()
    .add(("Access-Control-Allow-Origin", "*"))
    .add(("Access-Control-Expose-Headers", "*"))

    // Number of seconds a browser can cache the cors allows.
    // https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Access-Control-Max-Age
    // FF caps this at 24 hours, and is the most permissive there, so that's what we'll use.
    // Does this mean that my Cache-Control max-age is truncated to this value? That would be sad.
    .add(("Access-Control-Max-Age", "86400"))
}

// Before browsers will post data to a server, they make a CORS OPTIONS request to see if that's OK.
// This responds to that request to let the client know this request is allowed.
async fn cors_preflight_allow() -> HttpResponse {
    HttpResponse::NoContent()
        .append_header(("Access-Control-Allow-Methods", "OPTIONS, GET, PUT, HEAD"))
        .body("")
}


const MAX_ITEM_SIZE: usize = 1024 * 32; 
const PLAINTEXT: &'static str = "text/plain; charset=utf-8";




/// A type implementing ResponseError that can hold any kind of std::error::Error.
#[derive(Debug)]
pub(crate) struct Error {
    inner: Box<dyn std::error::Error + 'static>
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> std::result::Result<(), fmt::Error> { 
        self.inner.fmt(formatter)
    }
}

impl actix_web::error::ResponseError for Error {}

impl <E> From<E> for Error
where E: Into<Box<dyn std::error::Error + 'static>>
{
    fn from(inner: E) -> Self {
        Error{
            inner: inner.into()
        }
    }
}

/// An Error that is also Send, required in some cases:
#[derive(Debug)]
pub struct SendError {
    inner: Box<dyn std::error::Error + Send + 'static>
}

impl Into<Box<dyn std::error::Error>> for SendError {
    fn into(self) -> Box<dyn std::error::Error> {
        self.inner
    }
}

impl fmt::Display for SendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> std::result::Result<(), fmt::Error> { 
        self.inner.fmt(formatter)
    }
}

impl actix_web::error::ResponseError for SendError {}

impl <E> From<E> for SendError
where E: std::error::Error + Send + 'static
{
    fn from(err: E) -> Self {
        Self{
            inner: Box::new(err)
        }
    }
}

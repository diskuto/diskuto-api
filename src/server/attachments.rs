//! Functions for dealing w/ GET/POST/HEAD of /u/:userID/i/:signature/files/* endpoints.
//!
//! These endpoints are used both by the plain HTML and the client.
//! And, I suppose they could also be considered part of the REST API.


use std::io::{BufReader, BufWriter, Seek, SeekFrom};

use actix_web::{HttpRequest, HttpResponse, Responder, http::{header::{self, CONTENT_LENGTH}}, web::{Data, Path, Payload}};
use anyhow::Context;
use futures::{AsyncSeekExt, AsyncWriteExt, StreamExt};
use mime_guess::mime;
use sodiumoxide::crypto::hash::sha512;
use tempfile::tempfile;
use log::{debug};

use crate::{backend::{SHA512, Signature, UserID}, server::html::not_found};

use super::{AppData, Error, PLAINTEXT};

pub(crate) async fn get_file(
    req: HttpRequest,
    data: Data<AppData>,
    path: Path<(UserID, Signature, String)>,
) -> Result<HttpResponse, Error> {
    let (user_id, signature, file_name) = path.into_inner();
    let backend = data.backend_factory.open()?;

    let contents = backend.get_contents(user_id, signature, file_name.as_str())?;
    let contents = match contents {
        None => return not_found().await,
        Some(c) => c,
    };

    let mut mime_type = mime_guess::from_path(&file_name).first_or_octet_stream();

    // FeoBlog is not meant to be a general web server.
    // Plus, since the client also runs in the browser, any mime type that can run JavaScript
    // could exfiltrate private keys.
    // Javascript, obviously. But HTML and SVG(!!!) can embed JavaScript.
    if !safe_type(&mime_type) {
        mime_type = mime::APPLICATION_OCTET_STREAM;
    }

    let mime_string = mime_type.to_string();
    let response = HttpResponse::Ok()
        .content_type(mime_string)

        // no_chunking() sets the content-length, so this is redundant:
        // .set_header(CONTENT_LENGTH, contents.size)
        .no_chunking(contents.size)
        .streaming(contents.stream);

        // Note: Above we could've used a SizedStream, but it explicitly requires an actix::Error, not an Into<actix::Error>,
        // as streaming does. But actix::Error is not Send, which is required by blocking::Unblock.

    Ok(response)
}

// An allow-list for types we know can't embed JavaScript:
fn safe_type(mime_type: &mime_guess::Mime) -> bool {
    return match (mime_type.type_().as_str(), mime_type.subtype().as_str()) {
        ("text", "plain") => true,
        ("image", "gif") => true,
        ("image", "jpeg") => true,
        ("image", "png") => true,
        ("audio", "mpeg") => true,
        ("audio", "ogg") => true,
        
        // NO: javascript, HTML, SVG, others.
        // TODO: Consider a content-security-policy here to allow SVG?
        _ => false,
    }

}

pub(crate) async fn put_file(
    data: Data<AppData>,
    path: Path<(UserID, Signature, String)>,
    req: HttpRequest,
    mut body: Payload,
) -> Result<HttpResponse, Error> {
    let (user_id, signature, file_name) = path.into_inner();
    let backend = data.backend_factory.open()?;

    let metadata = backend.get_attachment_meta(&user_id, &signature, &file_name)?;
    // Drop our pooled connection while we wait for bytes, which could be slow:
    drop(backend);

    let metadata = match metadata {
        Some(d) => d,
        None => {
            // note: NO drain() here.
            // see: https://stackoverflow.com/questions/14250991/is-it-acceptable-for-a-server-to-send-a-http-response-before-the-entire-request
            // regarding HTTP errros. This is an error condition anyway.

            // If we don't yet have the metadata for a file (provided in its Item), then you can't upload yet.
            return Ok(
                HttpResponse::Forbidden()
                .content_type(PLAINTEXT)
                .body("No such attachment for this Item, or no such Item.")
            );
        }
    };
    
    if metadata.exists {
        drain(body).await;
        return Ok(
            HttpResponse::Accepted()
            .content_type(PLAINTEXT)
            .body("Attachment already exists")
        );
    }

    if metadata.quota_exceeded {
        return Ok(
            HttpResponse::InsufficientStorage()
            .content_type(PLAINTEXT)
            .body("Uploading this attachment would voilate the users's quota.")
        );
    }

    let length = match req.headers().get("content-length") {
        Some(length) => length,
        None => {
            return Ok(
                HttpResponse::LengthRequired()
                .content_type(PLAINTEXT)
                .body("Must include length header.".to_string())
                // ... so that we can reject things that are the wrong size outright.
            );
        }
    };

    let size = length
        .to_str().context("Parsing http Length header")?
        .parse::<u64>().context("Parsing http Length header")?;

    if metadata.size != size {
        return Ok(
            HttpResponse::BadRequest()
            .content_type(PLAINTEXT)
            .body(format!("File should be {} bytes but received {}", metadata.size, size))
        ); 
    }

    // Collect the file bytes into a temp file so that we're not using the backend while we wait for the upload:


    let file = tempfile().context("Error opening temp file")?;

    // Unblock's default buffer for I/O is *8MiB*!?  32k at a time seems fine.
    let mut file = blocking::Unblock::with_capacity(32 * 1024, file);

    let mut written: u64 = 0;
    let mut hasher = sha512::State::new();

    debug!("Receiving and hashing file: {}", &file_name);
    
    while let Some(chunk) = body.next().await {
        let chunk = chunk.context("Error parsing chunk")?;

        file.write_all(&chunk).await?;
        written += chunk.len() as u64;
        hasher.update(&chunk);
        if written > size { break; }
    }

    if written != size {
        return Ok(
            HttpResponse::BadRequest()
            .body(format!(
                "Expected {} bytes but received {}",
                size,
                written
            ))
        );
    }

    let hash = SHA512::from_digest(hasher.finalize());
    if hash != metadata.hash {
        return Ok(
            HttpResponse::BadRequest()
            .content_type(PLAINTEXT)
            .body(format!(
                "Invalid data. Expected {}", metadata.hash
            ))
        );
    }
    debug!("Received correct hash: {}", &hash);

    // Got here, file upload is good.
    // Copy the file into the appropriate BLOB.
    // Need to start from the beginning:
    file.flush().await?;

    // Just grab the inner file to simplify types for the Backend:
    let mut file = file.into_inner().await;

    blocking::unblock(move || -> Result<(), anyhow::Error> {
        file.seek(SeekFrom::Start(0))?;
        let backend = data.backend_factory.open()?;
        backend.save_attachment(metadata.size, &metadata.hash, &mut file)?;
        Ok(())
    }).await?;

    return Ok(
        HttpResponse::Created()
        .body("")
    );
}


/// If you don't wait to read all the Payload bytes, Actix-Web may close
/// the connection before the client has sent them all. Then the client
/// may behave poorly.
/// 
/// 1. Chrome has a long pause.
/// 2. Deno fetch() dies on the next request.  
///    See: https://github.com/denoland/deno/issues/11513
/// 
/// SO, here we read the whole payload, even though we don't care what
/// they sent. This seems ripe for DoS but it just seems to be the way
/// browsers and HTTP clients work? 😢
pub(crate) async fn drain(mut payload: Payload) {
    while payload.next().await.is_some() {}
}

pub(crate) async fn head_file(
    data: Data<AppData>,
    path: Path<(UserID, Signature, String)>,
    req: HttpRequest,
) -> Result<HttpResponse, Error> {
    let (user_id, signature, file_name) = path.into_inner();
    let backend = data.backend_factory.open()?;

    let metadata = backend.get_attachment_meta(&user_id, &signature, &file_name)?;

    let metadata = match metadata {
        Some(d) => d,
        None => {
            // Note: a 404 doesn't necessarily mean that you can upload.
            // The item doesn't yet exist, you can't upload a file here.
            return Ok(
                HttpResponse::NotFound().finish()
            );
        }
    };
    
    if metadata.exists {
        // I'd love to set a content-length here, but apparently Actix just won't let you for a HEAD?
        // See: https://github.com/actix/actix-web/issues/1439
        // See workaround here: https://github.com/actix/examples/blob/master/forms/multipart-s3/src/main.rs#L67-L79
        let response = HttpResponse::Ok().finish();
        return Ok(response);
    }

    if metadata.quota_exceeded {
        let response = HttpResponse::InsufficientStorage()
            .body("Submitting this file would exceed this user's quota.");
        return Ok(response)
    }

    let response = HttpResponse::NotFound().finish();

    Ok(response)
}
//! Non-standard endpoints.
//!
//! These are not part of the documented FeoBlog standard, but are used by this
//! particular FeoBlog implementation to provide extra features.

use actix_web::{HttpResponse, web::{Path, self}, error::ErrorInternalServerError};

use crate::backend::UserID;

/// This is not really defined as part of the standard for FeoBlogs.
/// BUT, having a default user image is handy when implementing the Open Graph Protocol.
/// (... which is itself also not a strict requirement for a FeoBlog.)
pub(crate) async fn identicon_get(path: Path<UserID>) -> Result<HttpResponse, actix_web::Error> {
    let user_id = path.into_inner();
    let result = actix_web::web::block(move || identicon_get_sync(user_id)).await?;

    result
        .map_err(|_| ErrorInternalServerError("Couldn't render icon"))
        .map(|icon| {
            let bytes = web::Bytes::from(icon);
            HttpResponse::Ok().content_type("image/png").body(bytes)
        })
}

fn identicon_get_sync(user_id: UserID) -> Result<Vec<u8>, ()> {
    use identicon::{Identicon, Mode::IdenticonJS};

    // Note: Must be >=16 bytes, but userIDs are bigger:
    let icon = Identicon::new(user_id.bytes())
        .mode(IdenticonJS(Default::default()))
        .background_rgb(255, 255, 255)
    ;

    let mut png = vec![];   
    icon.to_png(&mut png)
        // Can't actually reference the error type. Boo.
        .map_err(|_e| ())?;

    Ok(png)
}

//! REST endpoints.
//!
//! Note: some endpoints are in attachments.rs, since they're used by both REST & HTML views.

use actix_web::{HttpRequest, HttpResponse, web::{Data, Path, Payload, Query}, HttpResponseBuilder};
use anyhow::{Context, format_err};
use futures::StreamExt;
use logging_timer::timer;
use protobuf::Message;

use crate::{backend::{ItemDisplayRow, ItemRow, Signature, Timestamp, UserID}, protos::{Item, ItemList, ItemListEntry, ItemType, Item_oneof_item_type, ProtoValid}, server::{MAX_ITEM_SIZE, PLAINTEXT}};

use super::{AppData, Error, pagination::{Pagination, Paginator}, attachments::drain};


// Get the protobuf ItemList for items on the homepage.
pub(crate) async fn homepage_item_list(
    data: Data<AppData>,
    Query(pagination): Query<Pagination>,
) -> Result<HttpResponse, Error> {

    let mut paginator = Paginator::new(
        pagination,
        |row: ItemDisplayRow| -> Result<ItemListEntry,anyhow::Error> {
            let mut item = Item::new();
            item.merge_from_bytes(&row.item.item_bytes)?;
            Ok(item_to_entry(&item, &row.item.user, &row.item.signature))
        }, 
        |entry: &ItemListEntry| { 
            entry.get_item_type() == ItemType::POST
        }
    );
    // We're only holding ItemListEntries in memory, so we can up this limit and save some round trips.
    paginator.max_items = 1000;

    let backend = data.backend_factory.open()?;
    backend.homepage_items(paginator.time_span(), &mut paginator.callback())?;
    

    let mut list = ItemList::new();
    list.no_more_items = !paginator.has_more;
    list.items = protobuf::RepeatedField::from(paginator.into_items());
    Ok(
        proto_ok().body(list.write_to_bytes()?)
    )
}


pub(crate) async fn feed_item_list(
    data: Data<AppData>,
    path: Path<(UserID,)>,
    Query(pagination): Query<Pagination>,
) -> Result<HttpResponse, Error> {
    let (user_id,) = path.into_inner();
    let mut paginator = Paginator::new(
        pagination,
        |row: ItemDisplayRow| -> Result<ItemListEntry,anyhow::Error> {
            let mut item = Item::new();
            item.merge_from_bytes(&row.item.item_bytes)?;
            Ok(item_to_entry(&item, &row.item.user, &row.item.signature))
        }, 
        |_: &ItemListEntry| { true } // include all items
    );
    // We're only holding ItemListEntries in memory, so we can up this limit and
    // save some round trips.
    paginator.max_items = 1000;

    let backend = data.backend_factory.open()?;

    // Note: user_feed_items is doing a little bit of extra work to fetch
    // display_name, which we then throw away. We *could* make a more efficient
    // version that we use for just this case, but eh, reuse is nice.
    backend.user_feed_items(&user_id, paginator.time_span(), &mut paginator.callback())?;

    let mut list = ItemList::new();
    list.no_more_items = !paginator.has_more;
    list.items = protobuf::RepeatedField::from(paginator.into_items());
    Ok(
        proto_ok()
        .body(list.write_to_bytes()?)
    )
}

pub(crate) async fn user_item_list(
    data: Data<AppData>,
    path: Path<(UserID,)>,
    Query(pagination): Query<Pagination>,
) -> Result<HttpResponse, Error> {
    let (user_id,) = path.into_inner();
    let mut paginator = Paginator::new(
        pagination,
        |row: ItemRow| -> Result<ItemListEntry,anyhow::Error> {
            let mut item = Item::new();
            item.merge_from_bytes(&row.item_bytes)?;
            Ok(item_to_entry(&item, &row.user, &row.signature))
        }, 
        |_| { true } // include all items
    );
    // We're only holding ItemListEntries in memory, so we can up this limit and
    // save some round trips.
    paginator.max_items = 1000;

    let backend = data.backend_factory.open()?;

    // Note: user_feed_items is doing a little bit of extra work to fetch
    // display_name, which we then throw away. We *could* make a more efficient
    // version that we use for just this case, but eh, reuse is nice.
    backend.user_items(&user_id, paginator.time_span(), &mut paginator.callback())?;

    let mut list = ItemList::new();
    list.no_more_items = !paginator.has_more;
    list.items = protobuf::RepeatedField::from(paginator.into_items());
    Ok(
        proto_ok()
        .body(list.write_to_bytes()?)
    )
}

pub(crate) async fn item_reply_list(
    data: Data<AppData>,
    path: Path<(UserID, Signature)>,
    Query(pagination): Query<Pagination>,
) -> Result<HttpResponse, Error> {
    let (user_id, signature) = path.into_inner();
    let mut paginator = Paginator::new(
        pagination,
        |row: ItemRow| -> Result<ItemListEntry,anyhow::Error> {
            let mut item = Item::new();
            item.merge_from_bytes(&row.item_bytes)?;
            Ok(item_to_entry(&item, &row.user, &row.signature))
        }, 
        |_| { true } // include all items
    );
    // We're only holding ItemListEntries in memory, so we can up this limit and
    // save some round trips.
    paginator.max_items = 1000;

    let backend = data.backend_factory.open()?;

    // Note: user_feed_items is doing a little bit of extra work to fetch
    // display_name, which we then throw away. We *could* make a more efficient
    // version that we use for just this case, but eh, reuse is nice.
    backend.reply_items(&user_id, &signature, paginator.before(), &mut paginator.callback())?;

    let mut list = ItemList::new();
    list.no_more_items = !paginator.has_more;
    list.items = protobuf::RepeatedField::from(paginator.into_items());
    Ok(
        proto_ok()
        .body(list.write_to_bytes()?)
    )
}

/// Accepts a proto3 Item
/// Returns 201 if the PUT was successful.
/// Returns 202 if the item already exists.
/// Returns ??? if the user lacks permission to post.
/// Returns ??? if the signature is not valid.
/// Returns a text body message w/ OK/Error message.
pub(crate) async fn put_item(
    data: Data<AppData>,
    path: Path<(String, String,)>,
    req: HttpRequest,
    mut body: Payload,
) -> Result<HttpResponse, Error> 
{
    let _timer = timer!("put_item()");

    let (user_path, sig_path) = path.into_inner();
    let user = UserID::from_base58(user_path.as_str()).context("decoding user ID")?;
    let signature = Signature::from_base58(sig_path.as_str()).context("decoding signature")?;

    let length = match req.headers().get("content-length") {
        Some(length) => length,
        None => {
            return Ok(
                HttpResponse::LengthRequired()
                .content_type(PLAINTEXT)
                .body("Must include length header.".to_string())
                // ... so that we can reject things that are too large outright.
            );
        }
    };

    let length: usize = match length.to_str()?.parse() {
        Ok(length) => length,
        Err(_) => {
            return Ok(
                HttpResponse::BadRequest()
                .content_type(PLAINTEXT)
                .body("Error parsing Length header.".to_string())
            );
        },
    };

    if length > MAX_ITEM_SIZE {
        return Ok(
            HttpResponse::PayloadTooLarge()
            .content_type(PLAINTEXT)
            .body(format!("Item must be <= {} bytes", MAX_ITEM_SIZE))
        );
    }

    let mut backend = data.backend_factory.open()?;

    // If the content already exists, do nothing.
    if backend.user_item_exists(&user, &signature)? {
        // *sigh* this bug again. Should I handle this in middleware?
        drain(body).await;
        
        return Ok(
            HttpResponse::Accepted()
            .content_type(PLAINTEXT)
            .body("Item already exists")
        );
    }

    if !backend.user_known(&user)? {
        return Ok(
            HttpResponse::Forbidden()
            .content_type(PLAINTEXT)
            .body("Unknown user ID")
        )
    }
    
    let mut bytes: Vec<u8> = Vec::with_capacity(length);
    while let Some(chunk) = body.next().await {
        let chunk = chunk.context("Error parsing chunk")?;
        bytes.extend_from_slice(&chunk);
    }

    if !signature.is_valid(&user, &bytes) {
        // TODO: return invalid request here.
        Err(format_err!("Invalid signature"))?;
    }

    let mut item: Item = Item::new();
    item.merge_from_bytes(&bytes)?;
    item.validate()?;

    if item.timestamp_ms_utc > Timestamp::now().unix_utc_ms {
        return Ok(
            HttpResponse::BadRequest()
            .content_type(PLAINTEXT)
            .body("The Item's timestamp is in the future")
        )
    }

    if let Some(deny_reason) = backend.quota_check_item(&user, &bytes, &item)? {
        return Ok(
            HttpResponse::InsufficientStorage()
            .body(format!("{}", deny_reason))
        )
    }

    let message = format!("OK. Received {} bytes.", bytes.len());
    
    let row = ItemRow{
        user: user,
        signature: signature,
        timestamp: Timestamp{ unix_utc_ms: item.get_timestamp_ms_utc()},
        received: Timestamp::now(),
        item_bytes: bytes,
    };

    let timer = timer!("save_user_item");
    backend.save_user_item(&row, &item).context("Error saving user item")?;
    drop(timer);

    let response = HttpResponse::Created()
        .content_type(PLAINTEXT)
        .body(message);

    Ok(response)
}



/// Get the binary representation of the item.
///
/// `/u/{userID}/i/{sig}/proto3`
pub(crate) async fn get_item(
    data: Data<AppData>,
    path: Path<(UserID, Signature,)>,
) -> Result<HttpResponse, Error> {
    let (user_id, signature) = path.into_inner();
    let backend = data.backend_factory.open()?;
    let item = backend.user_item(&user_id, &signature)?;
    let item = match item {
        Some(item) => item,
        None => { 
            return Ok(
                HttpResponse::NotFound().body("No such item")
            );
        }
    };

    // We could in theory validate the bytes ourselves, but if a client is directly fetching the 
    // protobuf bytes via this endpoint, it's probably going to be so that it can verify the bytes
    // for itself anyway.
    Ok(
        proto_ok()
        .body(item.item_bytes)
    )

}

/// Get the latest profile we have for a user ID.
/// returns the signature in a "signature" header so clients can verify it.
pub(crate) async fn get_profile_item(
    data: Data<AppData>,
    path: Path<(UserID,)>,
) -> Result<HttpResponse, Error> {
    let (user_id,) = path.into_inner();
    let backend = data.backend_factory.open()?;
    let item = backend.user_profile(&user_id,)?;
    let item = match item {
        Some(item) => item,
        None => { 
            return Ok(
                HttpResponse::NotFound().body("No such item")
            );
        }
    };

    // We could in theory validate the bytes ourselves, but if a client is directly fetching the 
    // protobuf bytes via this endpoint, it's probably going to be so that it can verify the bytes
    // for itself anyway.
    Ok(
        proto_ok()
        .append_header(("signature", item.signature.to_base58()))
        .body(item.item_bytes)
    )
}


// --------------------------------------

// Start building a response w/ proto3 binary data.
fn proto_ok() -> HttpResponseBuilder {
    let mut builder = HttpResponse::Ok();
    builder.content_type("application/protobuf3");
    builder
}

fn item_to_entry(item: &Item, user_id: &UserID, signature: &Signature) -> ItemListEntry {
    let mut entry = ItemListEntry::new();
    entry.set_timestamp_ms_utc(item.timestamp_ms_utc);
    entry.set_signature({
        let mut sig = crate::protos::Signature::new();
        sig.set_bytes(signature.bytes().into());
        sig
    });
    entry.set_user_id({
        let mut uid = crate::protos::UserID::new();
        uid.set_bytes(user_id.bytes().into());
        uid
    });
    entry.set_item_type(
        match item.item_type {
            Some(Item_oneof_item_type::post(_)) => ItemType::POST,
            Some(Item_oneof_item_type::profile(_)) => ItemType::PROFILE,
            Some(Item_oneof_item_type::comment(_)) => ItemType::COMMENT,
            None => ItemType::UNKNOWN,
        }
    );

    entry
}

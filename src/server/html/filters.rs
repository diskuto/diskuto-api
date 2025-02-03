///! Filters for askama.

use std::borrow::Borrow;
use askama::Result;

use crate::{backend::{Signature, UserID}, markdown::{Options, ToHTML}};
use crate::backend::Timestamp;

pub(crate) fn markdown_with(s: &str, user_id: &UserID, signature: &Signature) -> Result<String> {
    Ok(
        s.md_to_html_with(Options{
            user_id: Some(user_id),
            signature: Some(signature),
        })
    )
}


// Seems filters always accept by reference:
// Except *sometimes* I don't get references.  Wat.  
// That took foreeeeever to find.  
pub(crate) fn with_offset(utc_ms: impl Borrow<i64>, offset_mins: impl Borrow<i32>) -> Result<String> {
    let timestamp = Timestamp{
        unix_utc_ms: *utc_ms.borrow(),
    };
    Ok(
        timestamp.format_with_offset(*offset_mins.borrow() as i16)
    )
}
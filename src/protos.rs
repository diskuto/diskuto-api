use std::borrow::Cow; 
use std::error::Error;
use std::fmt::{self, Display, Formatter};

mod diskuto;
pub use diskuto::*;

/// Since proto3 does not allow specifying required fields, we must do that
/// in our own validation here.
pub(crate) trait ProtoValid {
    fn validate(&self) -> Result<(), ValidationError> {
        match self.get_error() {
            None => Ok(()),
            Some(message) => Err(
                ValidationError{ message }
            ),
        }
    }

    /// Return the first known error with the proto, or None if it's valid.
    fn get_error(&self) -> Option<Cow<'static,str>>;
}

impl ProtoValid for Item {
    fn get_error(&self) -> Option<Cow<'static,str>> {

        // In proto3 we can't distinguish between 0 and not-present.
        // So, you can't express exactly the UTC start date, but you can
        // be 1ms on either side which seems good enough. :p
        if self.timestamp_ms_utc == 0 {
            return Some(
                "Timestamp is required".into()
            );
        }

        // TODO: Validations for specific item types.
        if self.has_profile() {
            let err = self.get_profile().get_error();
            if err.is_some() {
                return err;
            }
        }

        None
    }
}

impl ProtoValid for Profile {
    fn get_error(&self) -> Option<Cow<'static, str>> {

        for follow in self.get_follows() {
            if follow.get_user().get_bytes().len() != 32 {
                return Some("UserID.bytes must be 32 bytes".into())
            }
        }

        None
    }
}

#[derive(Debug)]
pub(crate) struct ValidationError {
    message: Cow<'static, str>,
}

impl Error for ValidationError {}

impl Display for ValidationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> { 
        write!(f, "Protobuf validation error: {}", self.message)
    }
}
#[derive(Debug)]
pub enum Error {
    // indicates that the received reply couldn't be decoded (e.g. caused by an aborted
    // connection)
    Utf8Decode,
    // indicates a Telegram error (e.g. a property is missing)
    Telegram(u32, String, Option<::objects::ResponseParameters>),
    // indicates some failure in CURL, missing network connection etc.
    TokioCurl(::tokio_curl::PerformError),
    // indicates a malformated reply, this should never happen unless the Telegram server has a
    // hard time
    Json(::serde_json::Error),
    // indicates an unknown error
    Unknown,
}

impl ::std::fmt::Display for Error {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        <Self as ::std::fmt::Debug>::fmt(self, f)
    }
}

impl ::std::error::Error for Error {
    fn description(&self) -> &str {
        "telebot error"
    }

    fn cause(&self) -> Option<&::std::error::Error> {
        use Error::*;
        match *self {
            TokioCurl(ref e) => Some(e),
            Json(ref e) => Some(e),
            _ => None,
        }
    }
}

impl From<::serde_json::Error> for Error {
    fn from(e: ::serde_json::Error) -> Error {
        Error::Json(e)
    }
}

impl From<::tokio_curl::PerformError> for Error {
    fn from(e: ::tokio_curl::PerformError) -> Error {
        Error::TokioCurl(e)
    }
}

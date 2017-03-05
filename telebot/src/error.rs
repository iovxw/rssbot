#[derive(Debug)]
pub enum Error {
    // indicates that the received reply couldn't be decoded (e.g. caused by an aborted
    // connection)
    UTF8Decode,
    // indicates a Telegram error (e.g. a property is missing)
    Telegram(String),
    // indicates some failure in CURL, missing network connection etc.
    TokioCurl,
    // indicates a malformated reply, this should never happen unless the Telegram server has a
    // hard time
    JSON,
    // indicates an unknown error
    Unknown,
}

error_chain! {
    errors {
        AlreadySubscribed

        NotSubscribed

        EOF {
            description("unexpected EOF")
        }

        TooManyRedirects {
            description("too many redirects")
        }

        EmptyFeed {
            description("feed is empty or not valid")
        }

        Http(code: u32) {
            description("unexpected HTTP response code")
            display("HTTP {} ({})", code, response_code(*code).unwrap_or("Unknown"))
        }

        DatabaseOpen(path: String) {
            description("failed to open database")
            display("failed to open database: '{}'", path)
        }

        DatabaseSave(path: String) {
            description("failed to save database")
            display("failed to save database: '{}'", path)
        }

        DatabaseFormat {
            description("illegal database format")
        }
    }
    links {
        Xml(::quick_xml::errors::Error, ::quick_xml::errors::ErrorKind);
    }
    foreign_links {
        Curl(::tokio_curl::PerformError);
        Utf8(::std::str::Utf8Error);
    }
}

fn response_code(code: u32) -> Option<&'static str> {
    match code {
        100 => Some("Continue"),
        101 => Some("Switching Protocols"),
        102 => Some("Processing"),
        200 => Some("OK"),
        201 => Some("Created"),
        202 => Some("Accepted"),
        203 => Some("Non-Authoritative Information"),
        204 => Some("No Content"),
        205 => Some("Reset Content"),
        206 => Some("Partial Content"),
        207 => Some("Multi-Status"),
        208 => Some("Already Reported"),
        226 => Some("IM Used"),
        300 => Some("Multiple Choices"),
        301 => Some("Moved Permanently"),
        302 => Some("Found"),
        303 => Some("See Other"),
        304 => Some("Not Modified"),
        305 => Some("Use Proxy"),
        306 => Some("Switch Proxy"),
        307 => Some("Temporary Redirect"),
        308 => Some("Permanent Redirect"),
        400 => Some("Bad Request"),
        401 => Some("Unauthorized"),
        402 => Some("Payment Required"),
        403 => Some("Forbidden"),
        404 => Some("Not Found"),
        405 => Some("Method Not Allowed"),
        406 => Some("Not Acceptable"),
        407 => Some("Proxy Authentication Required"),
        408 => Some("Request Timeout"),
        409 => Some("Conflict"),
        410 => Some("Gone"),
        411 => Some("Length Required"),
        412 => Some("Precondition Failed"),
        413 => Some("Payload Too Large"),
        414 => Some("URI Too Long"),
        415 => Some("Unsupported Media Type"),
        416 => Some("Range Not Satisfiable"),
        417 => Some("Expectation Failed"),
        418 => Some("I'm a teapot"),
        421 => Some("Misdirected Request"),
        422 => Some("Unprocessable Entity"),
        423 => Some("Locked"),
        424 => Some("Failed Dependency"),
        426 => Some("Upgrade Required"),
        428 => Some("Precondition Required"),
        429 => Some("Too Many Requests"),
        431 => Some("Request Header Fields Too Large"),
        451 => Some("Unavailable For Legal Reasons"),
        500 => Some("Internal Server Error"),
        501 => Some("Not Implemented"),
        502 => Some("Bad Gateway"),
        503 => Some("Service Unavailable"),
        504 => Some("Gateway Timeout"),
        505 => Some("HTTP Version Not Supported"),
        506 => Some("Variant Also Negotiates"),
        507 => Some("Insufficient Storage"),
        508 => Some("Loop Detected"),
        510 => Some("Not Extended"),
        511 => Some("Network Authentication Required"),
        // nginx
        444 => Some("No Response"),
        495 => Some("SSL Certificate Error"),
        496 => Some("SSL Certificate Required"),
        497 => Some("HTTP Request Sent to HTTPS Port"),
        499 => Some("Client Closed Request"),
        // CloudFlare
        520 => Some("Unknown Error"),
        521 => Some("Web Server Is Down"),
        522 => Some("Connection Timed Out"),
        523 => Some("Origin Is Unreachable"),
        524 => Some("A Timeout Occurred"),
        525 => Some("SSL Handshake Failed"),
        526 => Some("Invalid SSL Certificate"),
        _ => None,
    }
}

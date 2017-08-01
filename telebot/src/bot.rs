//! This is the actual Bot module. For ergonomic reasons there is a RcBot which composes the real
//! bot as an underlying field. You should always use RcBot.

use objects;
use error::Error;

use std::str;
use std::time::Duration;
use std::collections::HashMap;
use std::rc::Rc;
use std::cell::{RefCell, Cell};
use std::sync::{Arc, Mutex};

use curl::easy::{Easy, List, InfoType};
use tokio_curl::Session;
use tokio_core::reactor::{Handle, Interval};
use serde_json;
use serde_json::value::Value;
use futures::{Future, IntoFuture, Stream, stream};
use futures::sync::mpsc;
use futures::sync::mpsc::UnboundedSender;

/// A clonable, single threaded bot
///
/// The outer API gets implemented on RcBot
#[derive(Clone)]
pub struct RcBot {
    pub inner: Rc<Bot>,
}

impl RcBot {
    pub fn new<'a>(handle: Handle, key: &str) -> impl Future<Item = RcBot, Error = Error> + 'a {
        use functions::FunctionGetMe;
        let bot = RcBot { inner: Rc::new(Bot::new(handle, key)) };
        bot.get_me().send().map(|(mut bot, me)| {
            Rc::get_mut(&mut bot.inner).as_mut().unwrap().id = me.id;
            Rc::get_mut(&mut bot.inner).as_mut().unwrap().username = me.username.unwrap();
            bot
        })
    }
}

/// The main bot structure
pub struct Bot {
    pub key: String,
    pub handle: Handle,
    pub last_id: Cell<u32>,
    pub update_interval: Cell<u64>,
    pub handlers: RefCell<HashMap<String, UnboundedSender<(RcBot, objects::Message)>>>,
    pub session: Session,
    pub username: String,
    pub id: i64,
}

impl Bot {
    pub fn new(handle: Handle, key: &str) -> Bot {
        Bot {
            handle: handle.clone(),
            key: key.into(),
            last_id: Cell::new(0),
            update_interval: Cell::new(1000),
            handlers: RefCell::new(HashMap::new()),
            session: Session::new(handle.clone()),
            username: String::new(),
            id: 0,
        }
    }

    /// Creates a new request and adds a JSON message to it. The returned Future contains a the
    /// reply as a string.  This method should be used if no file is added because a JSON msg is
    /// always compacter than a formdata one.
    pub fn fetch_json<'a>(&self,
                          func: &str,
                          msg: &str)
                          -> impl Future<Item = String, Error = Error> + 'a {
        println!("Send JSON: {}", msg);

        let mut header = List::new();
        header.append("Content-Type: application/json").unwrap();

        let mut a = Easy::new();
        a.http_headers(header).unwrap();
        a.post_fields_copy(msg.as_bytes()).unwrap();
        a.post(true).unwrap();

        self._fetch(func, a)
    }

    /// calls cURL and parses the result for an error
    pub fn _fetch<'a>(&self,
                      func: &str,
                      mut a: Easy)
                      -> impl Future<Item = String, Error = Error> + 'a {
        let result = Arc::new(Mutex::new(Vec::new()));

        a.url(&format!("https://api.telegram.org/bot{}/{}", self.key, func)).unwrap();

        let r2 = result.clone();
        a.write_function(move |data| {
                r2.lock().unwrap().extend_from_slice(data);
                Ok(data.len())
            })
            .unwrap();

        // print debug information
        a.debug_function(|info, data| {
                match info {
                    InfoType::DataOut => {
                        println!("DataOut");
                    }
                    InfoType::Text => {
                        println!("Text");
                    }
                    InfoType::HeaderOut => {
                        println!("HeaderOut");
                    }
                    InfoType::SslDataOut => {
                        println!("SslDataOut");
                    }
                    _ => println!("something else"),
                }

                println!("{:?}", String::from_utf8_lossy(data));
            })
            .unwrap();
        //a.verbose(true).unwrap();
        //a.show_header(true).unwrap();

        self.session
            .perform(a)
            .map_err(|_| Error::TokioCurl)
            .map(move |_| {
                let response = result.lock().unwrap();
                String::from(str::from_utf8(&response).unwrap())
            })
            .and_then(|x| {
                // try to parse the result as a JSON and find the OK field.
                // If the ok field is true, then the string in "result" will be returned
                if let Ok(req) = serde_json::from_str::<Value>(&x) {
                    if let (Some(ok), res) = (req.get("ok").and_then(Value::as_bool),
                                              req.get("result")) {
                        if ok {
                            if let Some(result) = res {
                                let answer = serde_json::to_string(result).unwrap();

                                return Ok(answer);
                            }
                        }

                        match req.get("description").and_then(Value::as_str) {
                            Some(err) => Err(Error::Telegram(err.into())),
                            None => Err(Error::Telegram("Unknown".into())),
                        }
                    } else {
                        return Err(Error::JSON);
                    }
                } else {
                    return Err(Error::JSON);
                }
            })
    }
}

impl RcBot {
    /// Sets the update interval to an integer in milliseconds
    pub fn update_interval(self, interval: u64) -> RcBot {
        self.inner.update_interval.set(interval);

        self
    }

    /// Creates a new command and returns a stream which
    /// will yield a message when the command is send
    pub fn new_cmd(&self,
                   cmd: &str)
                   -> impl Stream<Item = (RcBot, objects::Message), Error = Error> {
        let (sender, receiver) = mpsc::unbounded();

        self.inner.handlers.borrow_mut().insert(cmd.into(), sender);

        receiver.map_err(|_| Error::Unknown)
    }

    /// Register a new commnd
    pub fn register<T>(&self, hnd: T)
        where T: Stream + 'static
    {
        self.inner.handle.spawn(hnd.for_each(|_| Ok(())).into_future().map(|_| ()).map_err(|_| ()));
    }

    /// The main update loop, the update function is called every update_interval milliseconds
    /// When an update is available the last_id will be updated and the message is filtered
    /// for commands
    /// The message is forwarded to the returned stream if no command was found
    pub fn get_stream<'a>(&'a self)
                          -> impl Stream<Item = (RcBot, objects::Update), Error = Error> + 'a {
        use functions::*;

        Interval::new(Duration::from_millis(self.inner.update_interval.get()),
                      &self.inner.handle)
            .unwrap()
            .map_err(|_| Error::Unknown)
            .and_then(move |_| self.get_updates().offset(self.inner.last_id.get()).send())
            .map(|(_, x)| {
                stream::iter(x.0
                    .into_iter()
                    .map(|x| Ok(x))
                    .collect::<Vec<Result<objects::Update, Error>>>())
            })
            .flatten()
            .and_then(move |x| {
                if self.inner.last_id.get() < x.update_id as u32 + 1 {
                    self.inner.last_id.set(x.update_id as u32 + 1);
                }

                Ok(x)
            })
            .filter_map(move |mut val| {
                let mut forward: Option<String> = None;

                if let Some(ref mut message) = val.message {
                    if let Some(text) = message.text.clone() {
                        let mut content = text.split_whitespace();
                        if let Some(cmd) = content.next() {
                            let s: Vec<&str> = cmd.split("@").take(2).collect();
                            if s.len() > 0 && (s.len() < 2 || s[1] == self.inner.username) &&
                               self.inner.handlers.borrow().contains_key(s[0]) {
                                message.text = Some(content.collect::<Vec<&str>>().join(" "));

                                forward = Some(s[0].into());
                            }
                        }
                    }
                }

                if let Some(cmd) = forward {
                    if let Some(sender) = self.inner.handlers.borrow_mut().get_mut(&cmd) {
                        sender.send((self.clone(), val.message.unwrap())).unwrap();
                    }
                    return None;
                } else {
                    return Some((self.clone(), val));
                }
            })
    }
}

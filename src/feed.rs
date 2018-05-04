use std;
use std::borrow::Cow;
use std::str;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use curl::easy::Easy;
use futures::prelude::*;
use quick_xml::events::attributes::Attributes;
use quick_xml::events::BytesStart;
use quick_xml::events::Event as XmlEvent;
use quick_xml::reader::Reader as XmlReader;
use regex::Regex;
use tokio_curl::Session;

use errors::*;

pub trait FromXml: Sized {
    fn from_xml<B: std::io::BufRead>(reader: &mut XmlReader<B>, start: &BytesStart)
        -> Result<Self>;
}

#[derive(Debug, Eq, PartialEq)]
enum AtomLink<'a> {
    Alternate(String),
    Source(String),
    Hub(String),
    Other(String, Cow<'a, str>),
}

fn parse_atom_link<'a, B: std::io::BufRead>(
    reader: &mut XmlReader<B>,
    attributes: Attributes<'a>,
) -> Result<Option<AtomLink<'a>>> {
    let mut href = None;
    let mut rel = None;
    for attribute in attributes {
        let attribute = attribute?;
        match reader.decode(attribute.key).as_ref() {
            "href" => href = Some(attribute.unescape_and_decode_value(reader)?),
            "rel" => rel = Some(reader.decode(attribute.value)),
            _ => (),
        }
    }
    Ok(href.map(move |href| {
        if let Some(rel) = rel {
            match &*rel {
                "alternate" => AtomLink::Alternate(href),
                "self" => AtomLink::Source(href),
                "hub" => AtomLink::Hub(href),
                _ => AtomLink::Other(href, rel),
            }
        } else {
            AtomLink::Alternate(href)
        }
    }))
}

fn skip_element<B: std::io::BufRead>(reader: &mut XmlReader<B>) -> Result<()> {
    let mut buf = Vec::new();
    loop {
        match reader.read_event(&mut buf) {
            Ok(XmlEvent::Start(_)) => {
                skip_element(reader)?;
            }
            Ok(XmlEvent::End(_)) | Ok(XmlEvent::Eof) => break,
            Err(err) => return Err(err.into()),
            _ => (),
        }
        buf.clear();
    }
    Ok(())
}

fn try_parse_text<'a, B: std::io::BufRead>(reader: &mut XmlReader<B>) -> Result<Option<String>> {
    let mut buf = Vec::new();
    let mut content: Option<String> = None;
    loop {
        match reader.read_event(&mut buf) {
            Ok(XmlEvent::Start(_)) => {
                skip_element(reader)?;
            }
            Ok(XmlEvent::Text(ref e)) => {
                let text = e.unescape_and_decode(reader)?;
                content = Some(text);
            }
            Ok(XmlEvent::CData(ref e)) => {
                let text = reader.decode(e).to_string();
                content = Some(text);
            }
            Ok(XmlEvent::End(_)) | Ok(XmlEvent::Eof) => break,
            Err(err) => return Err(err.into()),
            _ => (),
        }
        buf.clear();
    }
    Ok(content)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RSS {
    pub title: String,
    pub link: String,
    pub source: Option<String>,
    pub items: Vec<Item>,
}

impl FromXml for RSS {
    fn from_xml<B: std::io::BufRead>(
        reader: &mut XmlReader<B>,
        _start: &BytesStart,
    ) -> Result<Self> {
        let mut buf = Vec::new();
        let mut rss = RSS::default();
        loop {
            match reader.read_event(&mut buf) {
                Ok(XmlEvent::Empty(ref e)) => {
                    if reader.decode(e.local_name()) == "link" {
                        match parse_atom_link(reader, e.attributes())? {
                            Some(AtomLink::Alternate(link)) => rss.link = link,
                            Some(AtomLink::Source(link)) => rss.source = Some(link),
                            _ => {}
                        }
                    }
                }
                Ok(XmlEvent::Start(ref e)) => {
                    match reader.decode(e.local_name()).as_ref() {
                        "channel" => {
                            // RSS 0.9 1.0
                            let rdf = RSS::from_xml(reader, e)?;
                            rss.title = rdf.title;
                            rss.link = rdf.link;
                        }
                        "title" => {
                            if let Some(title) = try_parse_text(reader)? {
                                rss.title = title;
                            }
                        }
                        "link" => {
                            if let Some(link) = try_parse_text(reader)? {
                                // RSS
                                rss.link = link;
                            } else {
                                // ATOM
                                match parse_atom_link(reader, e.attributes())? {
                                    Some(AtomLink::Alternate(link)) => rss.link = link,
                                    Some(AtomLink::Source(link)) => rss.source = Some(link),
                                    _ => {}
                                }
                            }
                        }
                        "item" | "entry" => {
                            rss.items.push(Item::from_xml(reader, e)?);
                        }
                        _ => skip_element(reader)?,
                    }
                }
                Ok(XmlEvent::End(_)) | Ok(XmlEvent::Eof) => break,
                Err(err) => return Err(err.into()),
                _ => (),
            }
            buf.clear();
        }
        Ok(rss)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Item {
    pub title: Option<String>,
    pub link: Option<String>,
    pub id: Option<String>,
}

impl FromXml for Item {
    fn from_xml<B: std::io::BufRead>(
        reader: &mut XmlReader<B>,
        _start: &BytesStart,
    ) -> Result<Self> {
        let mut buf = Vec::new();
        let mut item = Item::default();
        loop {
            match reader.read_event(&mut buf) {
                Ok(XmlEvent::Empty(ref e)) => {
                    if reader.decode(e.name()).as_ref() == "link" {
                        if let Some(AtomLink::Alternate(link)) =
                            parse_atom_link(reader, e.attributes())?
                        {
                            item.link = Some(link);
                        }
                    }
                }
                Ok(XmlEvent::Start(ref e)) => {
                    match reader.decode(e.name()).as_ref() {
                        "title" => {
                            item.title = try_parse_text(reader)?;
                        }
                        "link" => {
                            if let Some(link) = try_parse_text(reader)? {
                                // RSS
                                item.link = Some(link);
                            } else if let Some(AtomLink::Alternate(link)) =
                                parse_atom_link(reader, e.attributes())?
                            {
                                // ATOM
                                item.link = Some(link);
                            }
                        }
                        "id" | "guid" => {
                            item.id = try_parse_text(reader)?;
                        }
                        _ => skip_element(reader)?,
                    }
                }
                Ok(XmlEvent::End(_)) | Ok(XmlEvent::Eof) => break,
                Err(err) => return Err(err.into()),
                _ => (),
            }
            buf.clear();
        }
        Ok(item)
    }
}

pub fn parse<B: std::io::BufRead>(reader: B) -> Result<RSS> {
    let mut reader = XmlReader::from_reader(reader);
    reader.trim_text(true);
    let mut buf = Vec::new();
    loop {
        match reader.read_event(&mut buf) {
            Ok(XmlEvent::Start(ref e)) => match reader.decode(e.name()).as_ref() {
                "rss" => continue,
                "channel" | "feed" | "rdf:RDF" => {
                    return RSS::from_xml(&mut reader, e);
                }
                _ => skip_element(&mut reader)?,
            },
            Ok(XmlEvent::Eof) => return Err(ErrorKind::EOF.into()),
            Err(err) => return Err(err.into()),
            _ => (),
        }
        buf.clear();
    }
}

fn set_url_relative_to_absolute(link: &mut String, host: &str) {
    match link.as_str() {
        _ if link.starts_with("//") => {
            let mut s = String::from("http:");
            s.push_str(link);
            *link = s;
        }
        _ if link.starts_with('/') => {
            let mut s = String::from(host);
            s.push_str(link);
            *link = s;
        }
        _ => (),
    }
}

fn fix_relative_url(mut rss: RSS, rss_link: &str) -> RSS {
    lazy_static! {
        static ref HOST: Regex = Regex::new(r"^(https?://[^/]+)").unwrap();
    }
    let rss_host = HOST.captures(rss_link)
        .map_or(rss_link, |r| r.get(0).unwrap().as_str());
    match rss.link.as_str() {
        "" | "/" => rss.link = rss_host.to_owned(),
        _ => set_url_relative_to_absolute(&mut rss.link, rss_host),
    }
    for item in &mut rss.items {
        if let Some(link) = item.link.as_mut() {
            set_url_relative_to_absolute(link, rss_host);
        }
    }

    rss
}

#[async]
fn make_request(
    session: Session,
    mut source: String,
    ua: String,
    mut recur_limit: usize,
) -> Result<(Vec<u8>, String, u32)> {
    let mut location: Option<String> = None;
    loop {
        if recur_limit == 0 {
            break Err(ErrorKind::TooManyRedirects.into());
        }
        let mut req = Easy::new();
        let buf = Arc::new(Mutex::new(Vec::new()));
        let location_buf = Arc::new(Mutex::new(String::new()));
        {
            let buf = Arc::clone(&buf);
            let location_buf = Arc::clone(&location_buf);
            req.get(true).unwrap();
            req.url(location.as_ref().unwrap_or(&source)).unwrap();
            req.accept_encoding("").unwrap(); // accept all encoding
            req.useragent(&ua).unwrap();
            req.timeout(Duration::from_secs(10)).unwrap();
            req.write_function(move |data| {
                buf.lock().unwrap().extend_from_slice(data);
                Ok(data.len())
            }).unwrap();
            req.header_function(move |data| {
                let header = String::from_utf8_lossy(data);
                let mut header = header.splitn(2, ':');
                if let (Some(k), Some(v)) = (header.next(), header.next()) {
                    if k == "Location" || k.to_lowercase() == "location" {
                        location_buf.lock().unwrap().push_str(v.trim());
                    }
                }
                true
            }).unwrap();
        }
        let mut resp = await!(session.perform(req))?;
        let response_code = resp.response_code().unwrap();
        ::std::mem::drop(resp); // make `buf` and `location_buf` strong count to zero
        if response_code == 301 {
            source = Arc::try_unwrap(location_buf).unwrap().into_inner().unwrap();
            location = None;
            recur_limit -= 1;
        } else if response_code == 302 {
            location = Some(Arc::try_unwrap(location_buf).unwrap().into_inner().unwrap());
            recur_limit -= 1;
        } else {
            let body = Arc::try_unwrap(buf).unwrap().into_inner().unwrap();
            break Ok((body, source, response_code));
        }
    }
}

pub fn fetch_feed<'a>(
    session: Session,
    ua: String,
    source: String,
) -> impl Future<Item = RSS, Error = Error> + 'a {
    fn is_vaild_link(link: &str) -> bool {
        link.starts_with("http://") || link.starts_with("https://")
    };
    make_request(session, source, ua, 10).and_then(move |(body, mut source, response_code)| {
        if response_code != 200 {
            return Err(ErrorKind::Http(response_code).into());
        }
        let mut rss = parse(body.as_slice())?;
        if rss == RSS::default() {
            return Err(ErrorKind::EmptyFeed.into());
        }
        if !is_vaild_link(&source) {
            source.insert_str(0, "http://");
        }
        if rss.source.is_none() || !is_vaild_link(rss.source.as_ref().unwrap()) {
            rss.source = Some(source.clone());
        }
        Ok(fix_relative_url(rss, &source))
    })
}

#[test]
fn test_atom03() {
    use std::io::Cursor;
    let s = include_str!("../tests/data/atom_0.3.xml");
    let r = parse(Cursor::new(s)).unwrap();
    assert_eq!(
        r,
        RSS {
            title: "atom_0.3.feed.title".into(),
            link: "atom_0.3.feed.link^href".into(),
            source: None,
            items: vec![
                Item {
                    title: Some("atom_0.3.feed.entry[0].title".into()),
                    link: Some("atom_0.3.feed.entry[0].link^href".into()),
                    id: Some("atom_0.3.feed.entry[0]^id".into()),
                },
                Item {
                    title: Some("atom_0.3.feed.entry[1].title".into()),
                    link: Some("atom_0.3.feed.entry[1].link^href".into()),
                    id: Some("atom_0.3.feed.entry[1]^id".into()),
                },
            ],
        }
    );
}

#[test]
fn test_atom10() {
    use std::io::Cursor;
    let s = include_str!("../tests/data/atom_1.0.xml");
    let r = parse(Cursor::new(s)).unwrap();
    assert_eq!(
        r,
        RSS {
            title: "atom_1.0.feed.title".into(),
            link: "http://example.com/blog_plain".into(),
            source: Some("http://example.com/blog/atom_1.0.xml".into()),
            items: vec![
                Item {
                    title: Some("atom_1.0.feed.entry[0].title".into()),
                    link: Some("http://example.com/blog/entry1_plain".into()),
                    id: Some("atom_1.0.feed.entry[0]^id".into()),
                },
                Item {
                    title: Some("atom_1.0.feed.entry[1].title".into()),
                    link: Some("http://example.com/blog/entry2".into()),
                    id: Some("atom_1.0.feed.entry[1]^id".into()),
                },
            ],
        }
    );
}

#[test]
fn test_rss09() {
    use std::io::Cursor;
    let s = include_str!("../tests/data/rss_0.9.xml");
    let r = parse(Cursor::new(s)).unwrap();
    assert_eq!(
        r,
        RSS {
            title: "rss_0.9.channel.title".into(),
            link: "rss_0.9.channel.link".into(),
            source: None,
            items: vec![
                Item {
                    title: Some("rss_0.9.item[0].title".into()),
                    link: Some("rss_0.9.item[0].link".into()),
                    id: None,
                },
                Item {
                    title: Some("rss_0.9.item[1].title".into()),
                    link: Some("rss_0.9.item[1].link".into()),
                    id: None,
                },
            ],
        }
    );
}

#[test]
fn test_rss091() {
    use std::io::Cursor;
    let s = include_str!("../tests/data/rss_0.91.xml");
    let r = parse(Cursor::new(s)).unwrap();
    assert_eq!(
        r,
        RSS {
            title: "rss_0.91.channel.title".into(),
            link: "rss_0.91.channel.link".into(),
            source: None,
            items: vec![
                Item {
                    title: Some("rss_0.91.channel.item[0].title".into()),
                    link: Some("rss_0.91.channel.item[0].link".into()),
                    id: None,
                },
                Item {
                    title: Some("rss_0.91.channel.item[1].title".into()),
                    link: Some("rss_0.91.channel.item[1].link".into()),
                    id: None,
                },
            ],
        }
    );
}

#[test]
fn test_rss092() {
    use std::io::Cursor;
    let s = include_str!("../tests/data/rss_0.92.xml");
    let r = parse(Cursor::new(s)).unwrap();
    assert_eq!(
        r,
        RSS {
            title: "rss_0.92.channel.title".into(),
            link: "rss_0.92.channel.link".into(),
            source: None,
            items: vec![
                Item {
                    title: Some("rss_0.92.channel.item[0].title".into()),
                    link: Some("rss_0.92.channel.item[0].link".into()),
                    id: None,
                },
                Item {
                    title: Some("rss_0.92.channel.item[1].title".into()),
                    link: Some("rss_0.92.channel.item[1].link".into()),
                    id: None,
                },
            ],
        }
    );
}

#[test]
fn test_rss093() {
    use std::io::Cursor;
    let s = include_str!("../tests/data/rss_0.93.xml");
    let r = parse(Cursor::new(s)).unwrap();
    assert_eq!(
        r,
        RSS {
            title: "rss_0.93.channel.title".into(),
            link: "rss_0.93.channel.link".into(),
            source: None,
            items: vec![
                Item {
                    title: Some("rss_0.93.channel.item[0].title".into()),
                    link: Some("rss_0.93.channel.item[0].link".into()),
                    id: None,
                },
                Item {
                    title: Some("rss_0.93.channel.item[1].title".into()),
                    link: Some("rss_0.93.channel.item[1].link".into()),
                    id: None,
                },
            ],
        }
    );
}

#[test]
fn test_rss094() {
    use std::io::Cursor;
    let s = include_str!("../tests/data/rss_0.94.xml");
    let r = parse(Cursor::new(s)).unwrap();
    assert_eq!(
        r,
        RSS {
            title: "rss_0.94.channel.title".into(),
            link: "rss_0.94.channel.link".into(),
            source: None,
            items: vec![
                Item {
                    title: Some("rss_0.94.channel.item[0].title".into()),
                    link: Some("rss_0.94.channel.item[0].link".into()),
                    id: Some("rss_0.94.channel.item[0].guid".into()),
                },
                Item {
                    title: Some("rss_0.94.channel.item[1].title".into()),
                    link: Some("rss_0.94.channel.item[1].link".into()),
                    id: Some("rss_0.94.channel.item[1].guid".into()),
                },
            ],
        }
    );
}

#[test]
fn test_rss10() {
    use std::io::Cursor;
    let s = include_str!("../tests/data/rss_1.0.xml");
    let r = parse(Cursor::new(s)).unwrap();
    assert_eq!(
        r,
        RSS {
            title: "rss_1.0.channel.title".into(),
            link: "rss_1.0.channel.link".into(),
            source: None,
            items: vec![
                Item {
                    title: Some("rss_1.0.item[0].title".into()),
                    link: Some("rss_1.0.item[0].link".into()),
                    id: None,
                },
                Item {
                    title: Some("rss_1.0.item[1].title".into()),
                    link: Some("rss_1.0.item[1].link".into()),
                    id: None,
                },
            ],
        }
    );
}

#[test]
fn test_rss20() {
    use std::io::Cursor;
    let s = include_str!("../tests/data/rss_2.0.xml");
    let r = parse(Cursor::new(s)).unwrap();
    assert_eq!(
        r,
        RSS {
            title: "rss_2.0.channel.title".into(),
            link: "rss_2.0.channel.link".into(),
            source: None,
            items: vec![
                Item {
                    title: Some("rss_2.0.channel.item[0].title".into()),
                    link: Some("rss_2.0.channel.item[0].link".into()),
                    id: Some("rss_2.0.channel.item[0].guid".into()),
                },
                Item {
                    title: Some("rss_2.0.channel.item[1].title".into()),
                    link: Some("rss_2.0.channel.item[1].link".into()),
                    id: Some("rss_2.0.channel.item[1].guid".into()),
                },
            ],
        }
    );
}

#[test]
fn test_rss_with_atom_ns() {
    use std::io::Cursor;
    let s = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">
<channel>
<atom:link href="self link" rel="self" />
</channel>
</rss>"#;
    let r = parse(Cursor::new(s)).unwrap();
    assert_eq!(r.source, Some("self link".into()));
}

#[test]
fn test_parse_atom_link() {
    use std::io::Cursor;
    let data = vec![
        r#"<link href="alternate href" />"#,
        r#"<link href="alternate href" rel="alternate" />"#,
        r#"<link href="self href" rel="self" />"#,
        r#"<link href="hub href" rel="hub" />"#,
        r#"<link href="other href" rel="other" />"#,
        r#"<link />"#,
    ];
    let results = vec![
        Some(AtomLink::Alternate("alternate href".into())),
        Some(AtomLink::Alternate("alternate href".into())),
        Some(AtomLink::Source("self href".into())),
        Some(AtomLink::Hub("hub href".into())),
        Some(AtomLink::Other(
            "other href".into(),
            Cow::Owned("other".into()),
        )),
        None,
    ];
    for (data, result) in data.iter().zip(results) {
        let mut reader = XmlReader::from_reader(Cursor::new(data));
        let mut buf = Vec::new();
        if let XmlEvent::Empty(e) = reader.read_event(&mut buf).unwrap() {
            let r = parse_atom_link(&mut reader, e.attributes()).unwrap();
            assert_eq!(r, result);
        }
    }
}

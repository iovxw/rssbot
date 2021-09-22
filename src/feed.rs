use std::borrow::Cow;
use std::cell::RefCell;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::str;

use lazy_static::lazy_static;
use quick_xml::events::attributes::Attributes;
use quick_xml::events::BytesStart;
use quick_xml::events::Event as XmlEvent;
use quick_xml::Reader as XmlReader;
use regex::Regex;
use serde::Deserialize;

trait FromXml: Sized {
    fn from_xml<B: std::io::BufRead>(
        bufs: &BufPool,
        reader: &mut XmlReader<B>,
        start: &BytesStart,
    ) -> quick_xml::Result<Self>;
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
) -> quick_xml::Result<Option<AtomLink<'a>>> {
    let mut href = None;
    let mut rel = None;
    for attribute in attributes {
        let attribute = attribute?;
        match &*reader.decode(attribute.key) {
            "href" => href = Some(attribute.unescape_and_decode_value(reader)?),
            "rel" => {
                rel = Some(reader.decode(if let Cow::Borrowed(s) = attribute.value {
                    s
                } else {
                    // Attrbute.value is always Borrowed
                    // https://docs.rs/quick-xml/0.18.1/src/quick_xml/events/attributes.rs.html#244
                    unreachable!()
                }))
            }
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

struct SkipThisElement;

impl FromXml for SkipThisElement {
    fn from_xml<B: std::io::BufRead>(
        bufs: &BufPool,
        reader: &mut XmlReader<B>,
        _start: &BytesStart,
    ) -> quick_xml::Result<Self> {
        let mut buf = bufs.pop();
        let mut depth = 1u64;
        loop {
            match reader.read_event(&mut buf) {
                Ok(XmlEvent::Start(_)) => depth += 1,
                Ok(XmlEvent::End(_)) if depth == 1 => break,
                Ok(XmlEvent::End(_)) => depth -= 1,
                Ok(XmlEvent::Eof) => break, // just ignore EOF
                Err(err) => return Err(err.into()),
                _ => (),
            }
            buf.clear();
        }
        Ok(SkipThisElement)
    }
}

impl FromXml for Option<u32> {
    fn from_xml<B: std::io::BufRead>(
        bufs: &BufPool,
        reader: &mut XmlReader<B>,
        _start: &BytesStart,
    ) -> quick_xml::Result<Self> {
        let mut buf = bufs.pop();
        let mut output = None;
        loop {
            match reader.read_event(&mut buf) {
                Ok(XmlEvent::Start(ref e)) => {
                    SkipThisElement::from_xml(bufs, reader, e)?;
                }
                Ok(XmlEvent::Text(ref e)) => {
                    let text = reader.decode(e);
                    output = text.parse().ok();
                }
                Ok(XmlEvent::End(_)) | Ok(XmlEvent::Eof) => break,
                Err(err) => return Err(err.into()),
                _ => (),
            }
            buf.clear();
        }
        Ok(output)
    }
}

impl FromXml for Option<String> {
    fn from_xml<B: std::io::BufRead>(
        bufs: &BufPool,
        reader: &mut XmlReader<B>,
        _start: &BytesStart,
    ) -> quick_xml::Result<Self> {
        let mut buf = bufs.pop();
        let mut content: Option<String> = None;
        loop {
            match reader.read_event(&mut buf) {
                Ok(XmlEvent::Start(ref e)) => {
                    SkipThisElement::from_xml(bufs, reader, e)?;
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
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Rss {
    pub title: String,
    #[serde(rename = "home_page_url", default)]
    pub link: String,
    #[serde(rename = "feed_url")]
    pub source: Option<String>,
    pub ttl: Option<u32>,
    pub items: Vec<Item>,
}

impl FromXml for Rss {
    fn from_xml<B: std::io::BufRead>(
        bufs: &BufPool,
        reader: &mut XmlReader<B>,
        _start: &BytesStart,
    ) -> quick_xml::Result<Self> {
        let mut buf = bufs.pop();
        let mut rss = Rss::default();
        let mut reading_rss_1_0_head = false;

        // http://purl.org/rss/1.0/modules/syndication/
        let mut sy_period: Option<SyPeriod> = None;
        let mut sy_freq: Option<u32> = None;

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
                    match &*reader.decode(e.local_name()) {
                        "channel" => {
                            // RSS 0.9 1.0
                            reading_rss_1_0_head = true;
                        }
                        "title" => {
                            if let Some(title) =
                                <Option<String> as FromXml>::from_xml(bufs, reader, e)?
                            {
                                rss.title = title;
                            }
                        }
                        "link" => {
                            if let Some(link) =
                                <Option<String> as FromXml>::from_xml(bufs, reader, e)?
                            {
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
                            rss.items.push(Item::from_xml(bufs, reader, e)?);
                        }
                        "ttl" => {
                            rss.ttl = <Option<u32> as FromXml>::from_xml(bufs, reader, e)?;
                        }
                        "updatePeriod" => {
                            sy_period = <Option<SyPeriod> as FromXml>::from_xml(bufs, reader, e)?;
                        }
                        "updateFrequency" => {
                            sy_freq = <Option<u32> as FromXml>::from_xml(bufs, reader, e)?;
                        }
                        _ => {
                            SkipThisElement::from_xml(bufs, reader, e)?;
                        }
                    }
                }
                Ok(XmlEvent::End(_)) if reading_rss_1_0_head => {
                    // reader.decode(e.local_name())? == "channel";
                    reading_rss_1_0_head = false;
                }
                Ok(XmlEvent::End(_)) | Ok(XmlEvent::Eof) => break,
                Err(err) => return Err(err.into()),
                _ => (),
            }
            buf.clear();
        }
        if rss.ttl.is_none() {
            let freq = sy_freq.unwrap_or(1); // 1 is the default value
            rss.ttl = match sy_period {
                Some(SyPeriod::Hourly) => Some(60 / freq),
                Some(SyPeriod::Daily) => Some((60 * 24) / freq),
                Some(SyPeriod::Weekly) => Some((60 * 24 * 7) / freq),
                Some(SyPeriod::Monthly) => Some((60 * 24 * 30) / freq),
                Some(SyPeriod::Yearly) => Some((60 * 24 * 365) / freq),
                None => None,
            };
        }
        Ok(rss)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Item {
    pub title: Option<String>,
    #[serde(rename = "url")]
    pub link: Option<String>,
    pub id: Option<String>,
}

impl FromXml for Item {
    fn from_xml<B: std::io::BufRead>(
        bufs: &BufPool,
        reader: &mut XmlReader<B>,
        _start: &BytesStart,
    ) -> quick_xml::Result<Self> {
        let mut buf = bufs.pop();
        let mut item = Item::default();
        loop {
            match reader.read_event(&mut buf) {
                Ok(XmlEvent::Empty(ref e)) => {
                    if reader.decode(e.name()) == "link" {
                        if let Some(AtomLink::Alternate(link)) =
                            parse_atom_link(reader, e.attributes())?
                        {
                            item.link = Some(link);
                        }
                    }
                }
                Ok(XmlEvent::Start(ref e)) => {
                    match &*reader.decode(e.name()) {
                        "title" => {
                            item.title = <Option<String> as FromXml>::from_xml(bufs, reader, e)?;
                        }
                        "link" => {
                            if let Some(link) =
                                <Option<String> as FromXml>::from_xml(bufs, reader, e)?
                            {
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
                            item.id = <Option<String> as FromXml>::from_xml(bufs, reader, e)?;
                        }
                        _ => {
                            SkipThisElement::from_xml(bufs, reader, e)?;
                        }
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

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
enum SyPeriod {
    Hourly,
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

impl FromXml for Option<SyPeriod> {
    fn from_xml<B: std::io::BufRead>(
        bufs: &BufPool,
        reader: &mut XmlReader<B>,
        _start: &BytesStart,
    ) -> quick_xml::Result<Self> {
        let mut buf = bufs.pop();
        let mut output = None;
        loop {
            match reader.read_event(&mut buf) {
                Ok(XmlEvent::Start(ref e)) => {
                    SkipThisElement::from_xml(bufs, reader, e)?;
                }
                Ok(XmlEvent::Text(ref e)) => {
                    let period = match &*reader.decode(e) {
                        "hourly" => SyPeriod::Hourly,
                        "daily" => SyPeriod::Daily,
                        "weekly" => SyPeriod::Weekly,
                        "monthly" => SyPeriod::Monthly,
                        "yearly" => SyPeriod::Yearly,
                        _ => continue, // ignore this error
                    };
                    output = Some(period);
                }
                Ok(XmlEvent::End(_)) | Ok(XmlEvent::Eof) => break,
                Err(err) => return Err(err.into()),
                _ => (),
            }
            buf.clear();
        }
        Ok(output)
    }
}

/// NOTE: This function doesn't check the syntax of feed, it only cares about performance
pub fn parse<B: std::io::BufRead>(reader: B) -> quick_xml::Result<Rss> {
    let mut reader = XmlReader::from_reader(reader);
    reader.trim_text(true);
    let bufs = BufPool::new(4, 512);
    let mut buf = bufs.pop();
    loop {
        match reader.read_event(&mut buf) {
            Ok(XmlEvent::Start(ref e)) => match &*reader.decode(e.name()) {
                "rss" => continue,
                "channel" | "feed" | "rdf:RDF" => {
                    return Rss::from_xml(&bufs, &mut reader, e);
                }
                _ => {
                    SkipThisElement::from_xml(&bufs, &mut reader, e)?;
                }
            },
            Ok(XmlEvent::Eof) => return Err(quick_xml::Error::UnexpectedEof("feed".to_string())),
            Err(err) => return Err(err.into()),
            _ => (),
        }
        buf.clear();
    }
}

fn url_relative_to_absolute(link: &mut String, host: &str) {
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

pub fn fix_relative_url(mut rss: Rss, rss_link: &str) -> Rss {
    lazy_static! {
        static ref HOST: Regex = Regex::new(r"^(https?://[^/]+)").unwrap();
    }
    let rss_host = HOST
        .captures(rss_link)
        .map_or(rss_link, |r| r.get(0).unwrap().as_str());
    match rss.link.as_str() {
        "" | "/" => rss.link = rss_host.to_owned(),
        _ => url_relative_to_absolute(&mut rss.link, rss_host),
    }
    for item in &mut rss.items {
        if let Some(link) = item.link.as_mut() {
            url_relative_to_absolute(link, rss_host);
        }
    }

    rss
}

struct BufPool {
    pool: Rc<RefCell<Vec<Vec<u8>>>>,
    capacity: usize,
}

impl BufPool {
    fn new(init_size: usize, capacity: usize) -> Self {
        BufPool {
            pool: Rc::new(RefCell::new(vec![Vec::with_capacity(capacity); init_size])),
            capacity,
        }
    }
    fn pop(&self) -> Buffer {
        let buf = self
            .pool
            .borrow_mut()
            .pop()
            .unwrap_or_else(|| Vec::with_capacity(self.capacity));
        Buffer {
            pool: self.pool.clone(),
            inner: buf,
        }
    }
}

struct Buffer {
    pool: Rc<RefCell<Vec<Vec<u8>>>>,
    inner: Vec<u8>,
}

impl Drop for Buffer {
    fn drop(&mut self) {
        self.pool
            .borrow_mut()
            .push(mem::replace(&mut self.inner, Vec::new()))
    }
}

impl Deref for Buffer {
    type Target = Vec<u8>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Buffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn encoding() {
        let s: &[u8] = &*include_bytes!("../tests/data/encoding.xml");
        let r = parse(Cursor::new(s)).unwrap();
        assert_eq!(r.title, "虎扑足球新闻")
    }

    #[test]
    fn atom03() {
        let s = include_str!("../tests/data/atom_0.3.xml");
        let r = parse(Cursor::new(s)).unwrap();
        assert_eq!(
            r,
            Rss {
                title: "atom_0.3.feed.title".into(),
                link: "atom_0.3.feed.link^href".into(),
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
                ..Rss::default()
            }
        );
    }

    #[test]
    fn atom10() {
        let s = include_str!("../tests/data/atom_1.0.xml");
        let r = parse(Cursor::new(s)).unwrap();
        assert_eq!(
            r,
            Rss {
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
                ..Rss::default()
            }
        );
    }

    #[test]
    fn rss09() {
        let s = include_str!("../tests/data/rss_0.9.xml");
        let r = parse(Cursor::new(s)).unwrap();
        assert_eq!(
            r,
            Rss {
                title: "rss_0.9.channel.title".into(),
                link: "rss_0.9.channel.link".into(),
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
                ..Rss::default()
            }
        );
    }

    #[test]
    fn rss091() {
        let s = include_str!("../tests/data/rss_0.91.xml");
        let r = parse(Cursor::new(s)).unwrap();
        assert_eq!(
            r,
            Rss {
                title: "rss_0.91.channel.title".into(),
                link: "rss_0.91.channel.link".into(),
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
                ..Rss::default()
            }
        );
    }

    #[test]
    fn rss092() {
        let s = include_str!("../tests/data/rss_0.92.xml");
        let r = parse(Cursor::new(s)).unwrap();
        assert_eq!(
            r,
            Rss {
                title: "rss_0.92.channel.title".into(),
                link: "rss_0.92.channel.link".into(),
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
                ..Rss::default()
            }
        );
    }

    #[test]
    fn rss093() {
        let s = include_str!("../tests/data/rss_0.93.xml");
        let r = parse(Cursor::new(s)).unwrap();
        assert_eq!(
            r,
            Rss {
                title: "rss_0.93.channel.title".into(),
                link: "rss_0.93.channel.link".into(),
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
                ..Rss::default()
            }
        );
    }

    #[test]
    fn rss094() {
        let s = include_str!("../tests/data/rss_0.94.xml");
        let r = parse(Cursor::new(s)).unwrap();
        assert_eq!(
            r,
            Rss {
                title: "rss_0.94.channel.title".into(),
                link: "rss_0.94.channel.link".into(),
                ttl: Some(100),
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
                ..Rss::default()
            }
        );
    }

    #[test]
    fn rss10() {
        let s = include_str!("../tests/data/rss_1.0.xml");
        let r = parse(Cursor::new(s)).unwrap();
        assert_eq!(
            r,
            Rss {
                title: "rss_1.0.channel.title".into(),
                link: "rss_1.0.channel.link".into(),
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
                ..Rss::default()
            }
        );
    }

    #[test]
    fn rss20() {
        let s = include_str!("../tests/data/rss_2.0.xml");
        let r = parse(Cursor::new(s)).unwrap();
        assert_eq!(
            r,
            Rss {
                title: "rss_2.0.channel.title".into(),
                link: "rss_2.0.channel.link".into(),
                ttl: Some(100),
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
                ..Rss::default()
            }
        );
    }

    #[test]
    fn rss_with_atom_ns() {
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
    fn atom_link_parsing() {
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

    #[test]
    fn empty_input() {
        let r = parse(Cursor::new(&[])).unwrap_err();
        assert!(matches!(r, quick_xml::Error::UnexpectedEof(s) if s == "feed" ))
    }

    #[test]
    fn ttl_sy() {
        let sy_input = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:sy="http://purl.org/rss/1.0/modules/syndication/">
<channel>
<sy:updatePeriod>hourly</sy:updatePeriod>
<sy:updateFrequency>6</sy:updateFrequency>
</channel>
</rss>"#;
        let sy_output = parse(Cursor::new(sy_input)).unwrap();
        assert_eq!(sy_output.ttl, Some(10));
    }

    #[test]
    fn ttl_sy_default_freq() {
        let sy_input = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:sy="http://purl.org/rss/1.0/modules/syndication/">
<channel>
<sy:updatePeriod>daily</sy:updatePeriod>
</channel>
</rss>"#;
        let sy_output = parse(Cursor::new(sy_input)).unwrap();
        assert_eq!(sy_output.ttl, Some(60 * 24));
    }

    #[test]
    fn ttl() {
        let input = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
<channel>
<ttl>
    42
</ttl>
</channel>
</rss>"#;
        let output = parse(Cursor::new(input)).unwrap();
        assert_eq!(output.ttl, Some(42));
    }

    #[test]
    fn ttl_priority() {
        let input = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:sy="http://purl.org/rss/1.0/modules/syndication/">
<channel>
<sy:updatePeriod>hourly</sy:updatePeriod>
<sy:updateFrequency>1</sy:updateFrequency>
<ttl>42</ttl>
</channel>
</rss>"#;
        let output = parse(Cursor::new(input)).unwrap();
        assert_eq!(output.ttl, Some(42));
    }

    // https://github.com/tafia/quick-xml/issues/311
    #[test]
    fn cdata_compatibility() {
        const CHARACTERS: &str = r#""'<>&"#;
        let input = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:sy="http://purl.org/rss/1.0/modules/syndication/">
<channel>
<title><![CDATA[{}]]></title>
</channel>
</rss>"#,
            CHARACTERS
        );
        let r = parse(Cursor::new(input)).unwrap();
        assert_eq!(
            r,
            Rss {
                title: CHARACTERS.into(),
                link: "".into(),
                ttl: None,
                source: None,
                items: vec![],
            }
        );
    }
}

use std::io::Cursor;
use std::io::Write;

use chrono::Local;
use quick_xml::events::attributes::Attribute;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::writer::Writer;

use data::Feed;
use errors::*;

pub fn to_opml(feeds: Vec<Feed>) -> String {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    let decl = BytesDecl::new(b"1.0", Some(b"UTF-8"), None);
    writer.write_event(Event::Decl(decl)).unwrap();

    with_tag(
        &mut writer,
        b"opml",
        &mut [Attribute::from(("version", "2.0")).into()],
        |writer| {
            with_tag(writer, b"head", &mut [], |writer| {
                with_tag(writer, b"title", &mut [], |writer| {
                    let text = BytesText::borrowed(b"Exported from RSSBot");
                    writer.write_event(Event::Text(text))?;
                    Ok(())
                })?;
                with_tag(writer, b"dateCreated", &mut [], |writer| {
                    // e.g. Thu, 02 Nov 2017 18:08:24 CST
                    let time = Local::now().format("%a, %d %b %Y %T %Z");
                    let text = BytesText::owned(time.to_string().into_bytes());
                    writer.write_event(Event::Text(text))?;
                    Ok(())
                })?;
                with_tag(writer, b"docs", &mut [], |writer| {
                    let text = BytesText::borrowed(b"http://www.opml.org/spec2");
                    writer.write_event(Event::Text(text))?;
                    Ok(())
                })
            })?;
            with_tag(writer, b"body", &mut [], move |writer| {
                for feed in feeds {
                    let mut outline = BytesStart::borrowed(b"outline", 7);
                    outline.push_attribute(Attribute::from(("type", "rss")));
                    outline.push_attribute(Attribute::from(("text", feed.title.as_str())));
                    outline.push_attribute(Attribute::from(("xmlUrl", feed.link.as_str())));
                    writer.write_event(Event::Empty(outline))?;
                }
                Ok(())
            })
        },
    ).unwrap();

    unsafe { String::from_utf8_unchecked(writer.into_inner().into_inner()) }
}

// type of `attrs` is for zero allocation
fn with_tag<'a, W, F>(
    writer: &mut Writer<W>,
    tag: &[u8],
    attrs: &mut [Option<Attribute<'a>>],
    then: F,
) -> Result<()>
where
    W: Write,
    F: FnOnce(&mut Writer<W>) -> Result<()>,
{
    let mut start = BytesStart::borrowed(tag, tag.len());
    for attr in attrs.iter_mut() {
        start.push_attribute(attr.take().unwrap());
    }
    writer.write_event(Event::Start(start)).unwrap();
    then(writer)?;
    let end = BytesEnd::borrowed(tag);
    writer.write_event(Event::End(end)).unwrap();
    Ok(())
}

#[test]
fn test_to_opml() {
    let mut feed1 = Feed::default();
    feed1.title = "title1".into();
    feed1.link = "link1".into();
    let mut feed2 = Feed::default();
    feed2.title = "title2".into();
    feed2.link = "link2".into();
    let feeds = vec![feed1, feed2];
    let r = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <opml version=\"2.0\">\
         <head>\
         <title>Exported from RSSBot</title>\
         <dateCreated>{}</dateCreated>\
         <docs>http://www.opml.org/spec2</docs>\
         </head>\
         <body>\
         <outline type=\"rss\" text=\"title1\" xmlUrl=\"link1\"/>\
         <outline type=\"rss\" text=\"title2\" xmlUrl=\"link2\"/>\
         </body>\
         </opml>",
        Local::now().format("%a, %d %b %Y %T %Z")
    );
    assert_eq!(to_opml(feeds), r);
}

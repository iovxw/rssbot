use std::collections::HashMap;

use rss;
use atom;

use errors::*;

fn atom_categories_to_rss_categories(categories: Vec<atom::Category>) -> Vec<rss::Category> {
    let mut result = Vec::with_capacity(categories.len());
    for category in categories {
        result.push(rss::Category {
            name: category.term,
            domain: category.scheme,
        });
    }
    result
}

fn atom_links_to_rss_link(mut links: Vec<atom::Link>) -> Option<String> {
    links.pop().map(|link| link.href)
}

fn atom_authors_to_rss_author(mut authors: Vec<atom::Person>) -> Option<String> {
    authors.pop().map(|author| author.name)
}

fn atom_entries_to_rss_items(entries: Vec<atom::Entry>) -> Vec<rss::Item> {
    let mut result = Vec::with_capacity(entries.len());
    for entry in entries {
        result.push(rss::Item {
            title: Some(entry.title),
            link: atom_links_to_rss_link(entry.links),
            description: entry.summary,
            author: atom_authors_to_rss_author(entry.authors),
            categories: atom_categories_to_rss_categories(entry.categories),
            comments: None,
            enclosure: None,
            guid: Some(rss::Guid {
                value: entry.id,
                is_permalink: false,
            }),
            pub_date: Some(entry.updated),
            source: entry.source.map(|source| {
                rss::Source {
                    url: atom_links_to_rss_link(source.links).unwrap_or_default(),
                    title: source.title,
                }
            }),
            content: entry.content.map(|content| match content {
                atom::Content::Text(s) |
                atom::Content::Html(s) => s,
                atom::Content::Xhtml(x) => x.content_str(),
            }),
            extensions: HashMap::new(),
            itunes_ext: None,
            dublin_core_ext: None,
        });
    }
    result
}

fn feed_to_channel(feed: atom::Feed) -> rss::Channel {
    rss::Channel {
        title: feed.title,
        link: atom_links_to_rss_link(feed.links).unwrap_or(feed.id),
        description: feed.subtitle.unwrap_or_default(),
        language: None,
        copyright: feed.rights,
        managing_editor: None,
        webmaster: None,
        pub_date: None,
        last_build_date: Some(feed.updated),
        categories: atom_categories_to_rss_categories(feed.categories),
        generator: feed.generator.map(|generator| generator.name),
        docs: None,
        cloud: None,
        ttl: None,
        image: None,
        text_input: None,
        skip_hours: Vec::new(),
        skip_days: Vec::new(),
        items: atom_entries_to_rss_items(feed.entries),
        extensions: HashMap::new(),
        itunes_ext: None,
        dublin_core_ext: None,
        namespaces: HashMap::new(),
    }
}

pub fn parse(s: &str) -> Result<rss::Channel> {
    s.parse::<rss::Channel>()
        .or_else(|err| match err {
            rss::Error::Xml(err) |
            rss::Error::XmlParsing(err, _) => Err(ErrorKind::Xml(err).into()),
            _ => {
                if s.contains("<channel>") {
                    Err(format!("{}", err).into())
                } else {
                    s.parse::<atom::Feed>()
                        .map(feed_to_channel)
                        .map_err(|err| if s.contains("<entry>") {
                            err.into()
                        } else {
                            ErrorKind::Unknown.into()
                        })
                }
            }
        })
}

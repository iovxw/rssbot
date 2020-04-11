use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::hash::{BuildHasherDefault, Hash, Hasher};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use serde_json;
use thiserror::Error;

use crate::feed;

#[derive(Error, Debug)]
pub enum DataError {
    #[error("io error")]
    Io(#[from] std::io::Error),
    #[error("json error")]
    Json(#[from] serde_json::Error),
}

fn gen_hash<T: Hash>(t: &T) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::default();
    t.hash(&mut hasher);
    hasher.finish()
}

type FeedId = u64;
type SubscriberId = i64;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Feed {
    pub link: String,
    pub title: String,
    pub down_time: Option<SystemTime>,
    pub subscribers: HashSet<SubscriberId, Size64>,
    pub ttl: Option<u32>,
    hash_list: Vec<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Hub {
    pub callback: String,
    pub secret: String,
}

#[derive(Debug)]
pub struct Database {
    path: PathBuf,
    feeds: HashMap<FeedId, Feed, Size64>,
    subscribers: HashMap<SubscriberId, HashSet<FeedId, Size64>, Size64>,
}

impl Database {
    pub fn create(path: PathBuf) -> Result<Database, DataError> {
        let result = Database {
            path,
            feeds: HashMap::with_hasher(Size64::default()),
            subscribers: HashMap::with_hasher(Size64::default()),
        };

        result.save()?;

        Ok(result)
    }

    pub fn open(path: PathBuf) -> Result<Database, DataError> {
        if path.exists() {
            let f = File::open(&path)?;
            let feeds_list: Vec<Feed> = serde_json::from_reader(&f)?;

            let mut feeds = HashMap::with_capacity_and_hasher(feeds_list.len(), Size64::default());
            let mut subscribers = HashMap::with_hasher(Size64::default());

            for feed in feeds_list {
                let feed_id = gen_hash(&feed.link);
                for subscriber in &feed.subscribers {
                    let subscribed_feeds = subscribers
                        .entry(subscriber.to_owned())
                        .or_insert_with(HashSet::default);
                    subscribed_feeds.insert(feed_id);
                }
                feeds.insert(feed_id, feed);
            }

            Ok(Database {
                path,
                feeds,
                subscribers,
            })
        } else {
            Database::create(path)
        }
    }

    pub fn all_feeds(&self) -> Vec<Feed> {
        self.feeds.iter().map(|(_, v)| v.clone()).collect()
    }

    pub fn all_subscribers(&self) -> Vec<SubscriberId> {
        self.subscribers.iter().map(|(k, _)| *k).collect()
    }

    pub fn subscribed_feeds(&self, subscriber: SubscriberId) -> Option<Vec<Feed>> {
        self.subscribers.get(&subscriber).map(|feeds| {
            feeds
                .iter()
                .map(|feed_id| &self.feeds[feed_id])
                .cloned()
                .collect()
        })
    }

    pub fn get_or_update_down_time(&mut self, rss_link: &str) -> Duration {
        let feed_id = gen_hash(&rss_link);
        let feed = self.feeds.get_mut(&feed_id).unwrap();
        let now = SystemTime::now();
        if let Some(t) = feed.down_time {
            now.duration_since(t).unwrap_or_default()
        } else {
            feed.down_time = Some(now);
            Duration::default()
        }
    }

    pub fn reset_down_time(&mut self, rss_link: &str) {
        let feed_id = gen_hash(&rss_link);
        let feed = self.feeds.get_mut(&feed_id).unwrap();
        feed.down_time = None;
    }

    pub fn is_subscribed(&self, subscriber: SubscriberId, rss_link: &str) -> bool {
        self.subscribers
            .get(&subscriber)
            .map(|feeds| feeds.contains(&gen_hash(&rss_link)))
            .unwrap_or(false)
    }

    pub fn subscribe(&mut self, subscriber: SubscriberId, rss_link: &str, rss: &feed::Rss) -> bool {
        let feed_id = gen_hash(&rss_link);
        {
            let subscribed_feeds = self
                .subscribers
                .entry(subscriber)
                .or_insert_with(HashSet::default);
            if !subscribed_feeds.insert(feed_id) {
                return false;
            }
        }
        {
            let feed = self.feeds.entry(feed_id).or_insert_with(|| Feed {
                link: rss_link.to_owned(),
                title: rss.title.to_owned(),
                down_time: None,
                ttl: rss.ttl,
                hash_list: rss.items.iter().map(gen_item_hash).collect(),
                subscribers: HashSet::default(),
            });
            feed.subscribers.insert(subscriber);
        }
        self.save().unwrap_or_default();
        true
    }

    pub fn unsubscribe(&mut self, subscriber: SubscriberId, rss_link: &str) -> Option<Feed> {
        let feed_id = gen_hash(&rss_link);

        let clear_subscriber;
        if let Some(subscribed_feeds) = self.subscribers.get_mut(&subscriber) {
            if subscribed_feeds.remove(&feed_id) {
                clear_subscriber = subscribed_feeds.is_empty();
            } else {
                return None;
            }
        } else {
            return None;
        }
        if clear_subscriber {
            self.subscribers.remove(&subscriber);
        }

        let result;
        let clear_feed;
        if let Some(feed) = self.feeds.get_mut(&feed_id) {
            if feed.subscribers.remove(&subscriber) {
                clear_feed = feed.subscribers.is_empty();
                result = feed.clone();
            } else {
                return None;
            }
        } else {
            return None;
        };
        if clear_feed {
            self.feeds.remove(&feed_id);
        }
        self.save().unwrap_or_default();
        Some(result)
    }

    pub fn delete_subscriber(&mut self, subscriber: SubscriberId) {
        self.subscribed_feeds(subscriber)
            .map(|feeds| {
                for feed in feeds {
                    let _ = self.unsubscribe(subscriber, &feed.link);
                }
            })
            .unwrap_or_default();
    }

    pub fn update_subscriber(&mut self, from: SubscriberId, to: SubscriberId) {
        let feeds = self.subscribers.remove(&from).unwrap();
        for feed_id in &feeds {
            let feed = self.feeds.get_mut(&feed_id).unwrap();
            feed.subscribers.remove(&from);
            feed.subscribers.insert(to);
        }
        self.subscribers.insert(to, feeds);
    }

    /// Update the feed in database, return updates
    pub fn update(&mut self, rss_link: &str, new_feed: feed::Rss) -> Vec<FeedUpdate> {
        let feed_id = gen_hash(&rss_link);
        if self.feeds.get(&feed_id).is_none() {
            return Vec::new();
        }

        self.reset_down_time(rss_link);
        let feed = self.feeds.get_mut(&feed_id).unwrap();

        let mut updates = Vec::new();
        let mut new_items = Vec::new();
        let mut new_hash_list = Vec::new();
        let items_len = new_feed.items.len();
        for item in new_feed.items {
            let hash = gen_item_hash(&item);
            if !feed.hash_list.contains(&hash) {
                new_hash_list.push(hash);
                new_items.push(item);
            }
        }
        if !new_items.is_empty() {
            updates.push(FeedUpdate::Items(new_items));

            let max_size = items_len * 2;
            let mut append: Vec<u64> = feed
                .hash_list
                .iter()
                .take(max_size - new_hash_list.len())
                .cloned()
                .collect();
            new_hash_list.append(&mut append);
            feed.hash_list = new_hash_list;
        }
        if new_feed.title != feed.title {
            updates.push(FeedUpdate::Title(new_feed.title.clone()));
            feed.title = new_feed.title;
        }
        feed.ttl = new_feed.ttl;
        if !updates.is_empty() {
            self.save().unwrap_or_default();
        }
        updates
    }

    pub fn save(&self) -> Result<(), DataError> {
        let feeds_list: Vec<&Feed> = self.feeds.iter().map(|(_id, feed)| feed).collect();
        let mut file = File::create(&self.path)?;
        if let Err(e) = serde_json::to_writer(&mut file, &feeds_list) {
            if e.is_io() {
                return Err(DataError::Io(e.into()));
            } else {
                unreachable!(e);
            };
        }
        Ok(())
    }
}

pub enum FeedUpdate {
    Items(Vec<feed::Item>),
    Title(String),
}

fn gen_item_hash(item: &feed::Item) -> u64 {
    item.id.as_ref().map(|id| gen_hash(&id)).unwrap_or_else(|| {
        let title = item.title.as_ref().map(|s| s.as_str()).unwrap_or_default();
        let link = item.link.as_ref().map(|s| s.as_str()).unwrap_or_default();
        gen_hash(&format!("{}{}", title, link))
    })
}

pub type Size64 = BuildHasherDefault<Size64Hasher>;

/// A specialized hasher for u64 and i64
///
/// WARNING: Do not use it for user-controlled input
#[derive(Default)]
pub struct Size64Hasher {
    finished: bool,
    value: u64,
}

impl Hasher for Size64Hasher {
    fn finish(&self) -> u64 {
        self.value
    }

    fn write(&mut self, _bytes: &[u8]) {
        panic!("only support u64 and i64");
    }

    fn write_u64(&mut self, i: u64) {
        assert!(
            !self.finished,
            "this is a special hasher, do not write twice"
        );
        self.value = i;
        self.finished = true;
    }

    fn write_i64(&mut self, i: i64) {
        self.write_u64(i as u64);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn size64hasher() {
        let mut h = Size64Hasher::default();
        h.write_i64(-42);
        assert_eq!(h.finish(), -42i64 as u64);
    }

    #[test]
    #[should_panic(expected = "this is a special hasher, do not write twice")]
    fn size64hasher_write_twice() {
        let mut h = Size64Hasher::default();
        h.write_u64(42);
        h.write_u64(42);
    }

    #[test]
    #[should_panic(expected = "only support u64 and i64")]
    fn size64hasher_other_types() {
        let mut h = Size64Hasher::default();
        h.write_u8(0);
    }
}

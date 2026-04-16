use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::hash::{BuildHasherDefault, Hash, Hasher};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use atomicwrites::{AtomicFile, OverwriteBehavior};
use serde::{Deserialize, Serialize};
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
        self.feeds.values().cloned().collect()
    }

    pub fn all_subscribers(&self) -> Vec<SubscriberId> {
        self.subscribers.keys().copied().collect()
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

    /// Return `None` if feed not found
    pub fn get_or_update_down_time(&mut self, rss_link: &str) -> Option<Duration> {
        let feed_id = gen_hash(&rss_link);
        let feed = self.feeds.get_mut(&feed_id)?;
        let now = SystemTime::now();
        if let Some(t) = feed.down_time {
            Some(now.duration_since(t).unwrap_or_default())
        } else {
            feed.down_time = Some(now);
            Some(Duration::default())
        }
    }

    pub fn reset_down_time(&mut self, rss_link: &str) -> bool {
        let feed_id = gen_hash(&rss_link);
        self.feeds
            .get_mut(&feed_id)
            .map(|feed| {
                feed.down_time = None;
            })
            .is_some()
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
            let subscribed_feeds = self.subscribers.entry(subscriber).or_default();
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

    pub fn delete_subscriber(&mut self, subscriber: SubscriberId) -> bool {
        self.subscribed_feeds(subscriber)
            .map(|feeds| {
                for feed in feeds {
                    let _ = self.unsubscribe(subscriber, &feed.link);
                }
            })
            .is_some()
    }

    pub fn update_subscriber(&mut self, from: SubscriberId, to: SubscriberId) -> bool {
        self.subscribers
            .remove(&from)
            .map(|feeds| {
                for feed_id in &feeds {
                    let feed = self.feeds.get_mut(feed_id).unwrap();
                    feed.subscribers.remove(&from);
                    feed.subscribers.insert(to);
                }
                self.subscribers.insert(to, feeds);
                self.save().unwrap_or_default();
            })
            .is_some()
    }

    /// Update the feed in database, return updates
    pub fn update(&mut self, rss_link: &str, new_feed: feed::Rss) -> Vec<FeedUpdate> {
        let feed_id = gen_hash(&rss_link);
        if !self.feeds.contains_key(&feed_id) {
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
        let feeds_list: Vec<&Feed> = self.feeds.values().collect();
        let file = AtomicFile::new(&self.path, OverwriteBehavior::AllowOverwrite);
        file.write(|file| serde_json::to_writer(file, &feeds_list))
            .map_err(|e| match e {
                atomicwrites::Error::Internal(e) => DataError::Io(e),
                atomicwrites::Error::User(e) => {
                    assert!(!e.is_io(), "unreachable code");
                    DataError::Io(e.into())
                }
            })?;
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum FeedUpdate {
    Items(Vec<feed::Item>),
    Title(String),
}

fn gen_item_hash(item: &feed::Item) -> u64 {
    item.id.as_ref().map(|id| gen_hash(&id)).unwrap_or_else(|| {
        let title = item.title.as_deref().unwrap_or_default();
        let link = item.link.as_deref().unwrap_or_default();
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
    use std::time::UNIX_EPOCH;

    const FEED_URL: &str = "https://example.com/feed.xml";

    fn temp_db_path() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "rssbot-data-test-{}-{}.json",
            std::process::id(),
            unique
        ))
    }

    fn make_item(id: &str, title: &str, link: &str) -> feed::Item {
        feed::Item {
            title: Some(title.to_owned()),
            link: Some(link.to_owned()),
            id: Some(id.to_owned()),
        }
    }

    fn make_rss(title: &str, ttl: Option<u32>, items: Vec<feed::Item>) -> feed::Rss {
        feed::Rss {
            title: title.to_owned(),
            link: "https://example.com/home".to_owned(),
            source: Some(FEED_URL.to_owned()),
            ttl,
            items,
        }
    }

    fn remove_file_if_exists(path: &PathBuf) {
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => panic!("failed to remove {}: {}", path.display(), err),
        }
    }

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

    #[test]
    fn persists_subscription_lifecycle() {
        let path = temp_db_path();
        remove_file_if_exists(&path);

        let mut db = Database::open(path.clone()).unwrap();
        let rss = make_rss("Feed", Some(30), vec![make_item("item-1", "Item 1", "https://example.com/1")]);

        assert!(db.subscribe(1, FEED_URL, &rss));
        assert!(!db.subscribe(1, FEED_URL, &rss));
        assert!(db.is_subscribed(1, FEED_URL));
        assert_eq!(db.subscribed_feeds(1).unwrap().len(), 1);

        let reopened = Database::open(path.clone()).unwrap();
        assert!(reopened.is_subscribed(1, FEED_URL));
        assert_eq!(reopened.all_subscribers(), vec![1]);
        assert_eq!(reopened.all_feeds().len(), 1);

        assert!(db.unsubscribe(1, FEED_URL).is_some());
        assert!(db.unsubscribe(1, FEED_URL).is_none());
        assert!(db.subscribed_feeds(1).is_none());
        assert!(db.all_feeds().is_empty());

        remove_file_if_exists(&path);
    }

    #[test]
    fn migrated_subscriber_is_saved_to_disk() {
        let path = temp_db_path();
        remove_file_if_exists(&path);

        let mut db = Database::open(path.clone()).unwrap();
        let rss = make_rss("Feed", Some(30), vec![make_item("item-1", "Item 1", "https://example.com/1")]);

        assert!(db.subscribe(1, FEED_URL, &rss));
        assert!(db.update_subscriber(1, 2));
        assert!(!db.is_subscribed(1, FEED_URL));
        assert!(db.is_subscribed(2, FEED_URL));

        let reopened = Database::open(path.clone()).unwrap();
        assert!(!reopened.is_subscribed(1, FEED_URL));
        assert!(reopened.is_subscribed(2, FEED_URL));

        remove_file_if_exists(&path);
    }

    #[test]
    fn update_returns_new_items_and_title_changes() {
        let path = temp_db_path();
        remove_file_if_exists(&path);

        let mut db = Database::open(path.clone()).unwrap();
        let initial = make_rss(
            "Old title",
            Some(30),
            vec![make_item("item-1", "Old item", "https://example.com/1")],
        );
        assert!(db.subscribe(1, FEED_URL, &initial));

        let updates = db.update(
            FEED_URL,
            make_rss(
                "New title",
                Some(60),
                vec![
                    make_item("item-2", "New item", "https://example.com/2"),
                    make_item("item-1", "Old item", "https://example.com/1"),
                ],
            ),
        );

        assert_eq!(
            updates,
            vec![
                FeedUpdate::Items(vec![make_item("item-2", "New item", "https://example.com/2")]),
                FeedUpdate::Title("New title".to_owned()),
            ]
        );

        let stored_feed = db.subscribed_feeds(1).unwrap().pop().unwrap();
        assert_eq!(stored_feed.title, "New title");
        assert_eq!(stored_feed.ttl, Some(60));

        remove_file_if_exists(&path);
    }

    #[test]
    fn tracks_and_resets_downtime_for_known_feeds() {
        let path = temp_db_path();
        remove_file_if_exists(&path);

        let mut db = Database::open(path.clone()).unwrap();
        let rss = make_rss("Feed", Some(30), vec![make_item("item-1", "Item 1", "https://example.com/1")]);
        assert!(db.subscribe(1, FEED_URL, &rss));

        assert_eq!(db.get_or_update_down_time(FEED_URL), Some(Duration::default()));
        assert!(db.get_or_update_down_time(FEED_URL).is_some());
        assert!(db.reset_down_time(FEED_URL));
        assert!(db.feeds[&gen_hash(&FEED_URL)].down_time.is_none());
        assert!(!db.reset_down_time("https://example.com/missing.xml"));

        remove_file_if_exists(&path);
    }

    #[test]
    fn item_hash_prefers_id_and_falls_back_to_title_and_link() {
        let with_id_a = make_item("same-id", "A", "https://example.com/a");
        let with_id_b = make_item("same-id", "B", "https://example.com/b");
        assert_eq!(gen_item_hash(&with_id_a), gen_item_hash(&with_id_b));

        let without_id_a = feed::Item {
            id: None,
            title: Some("Title".to_owned()),
            link: Some("https://example.com/a".to_owned()),
        };
        let without_id_b = feed::Item {
            id: None,
            title: Some("Title".to_owned()),
            link: Some("https://example.com/a".to_owned()),
        };
        let without_id_c = feed::Item {
            id: None,
            title: Some("Other".to_owned()),
            link: Some("https://example.com/c".to_owned()),
        };

        assert_eq!(gen_item_hash(&without_id_a), gen_item_hash(&without_id_b));
        assert_ne!(gen_item_hash(&without_id_a), gen_item_hash(&without_id_c));
    }
}

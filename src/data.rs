use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::SystemTime;

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

type FeedID = u64;
type SubscriberID = i64;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Feed {
    pub link: String,
    pub title: String,
    pub down_time: Option<SystemTime>,
    pub subscribers: HashSet<SubscriberID>,
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
    feeds: HashMap<FeedID, Feed>,
    subscribers: HashMap<SubscriberID, HashSet<FeedID>>,
}

impl Database {
    pub fn create(path: PathBuf) -> Result<Database, DataError> {
        let feeds: HashMap<FeedID, Feed> = HashMap::new();
        let subscribers: HashMap<SubscriberID, HashSet<FeedID>> = HashMap::new();
        let result = Database {
            path: path,
            feeds: feeds,
            subscribers: subscribers,
        };

        result.save()?;

        Ok(result)
    }

    pub fn open(path: PathBuf) -> Result<Database, DataError> {
        if path.exists() {
            let f = File::open(&path)?;
            let feeds_list: Vec<Feed> = serde_json::from_reader(&f)?;

            let mut feeds: HashMap<FeedID, Feed> = HashMap::with_capacity(feeds_list.len());
            let mut subscribers: HashMap<SubscriberID, HashSet<FeedID>> = HashMap::new();

            for feed in feeds_list {
                let feed_id = gen_hash(&feed.link);
                for subscriber in &feed.subscribers {
                    let subscribed_feeds = subscribers
                        .entry(subscriber.to_owned())
                        .or_insert_with(HashSet::new);
                    subscribed_feeds.insert(feed_id);
                }
                feeds.insert(feed_id, feed);
            }

            Ok(Database {
                path: path,
                feeds: feeds,
                subscribers: subscribers,
            })
        } else {
            Database::create(path)
        }
    }

    pub fn all_feeds(&self) -> Vec<Feed> {
        self.feeds.iter().map(|(_, v)| v.clone()).collect()
    }

    pub fn all_subscribers(&self) -> Vec<SubscriberID> {
        self.subscribers.iter().map(|(k, _)| *k).collect()
    }

    pub fn subscribed_feeds(&self, subscriber: SubscriberID) -> Option<Vec<Feed>> {
        self.subscribers.get(&subscriber).map(|feeds| {
            feeds
                .iter()
                .map(|feed_id| &self.feeds[feed_id])
                .cloned()
                .collect()
        })
    }

    pub fn set_down_time(&mut self, rss_link: &str) -> SystemTime {
        let feed_id = gen_hash(&rss_link);
        let feed = self.feeds.get_mut(&feed_id).unwrap();
        *feed.down_time.get_or_insert_with(|| SystemTime::now())
    }

    pub fn reset_down_time(&mut self, rss_link: &str) {
        let feed_id = gen_hash(&rss_link);
        let feed = self.feeds.get_mut(&feed_id).unwrap();
        feed.down_time = None;
    }

    pub fn is_subscribed(&self, subscriber: SubscriberID, rss_link: &str) -> bool {
        self.subscribers
            .get(&subscriber)
            .map(|feeds| feeds.contains(&gen_hash(&rss_link)))
            .unwrap_or(false)
    }

    pub fn subscribe(&mut self, subscriber: SubscriberID, rss_link: &str, rss: &feed::Rss) -> bool {
        let feed_id = gen_hash(&rss_link);
        {
            let subscribed_feeds = self
                .subscribers
                .entry(subscriber)
                .or_insert_with(HashSet::new);
            if !subscribed_feeds.insert(feed_id) {
                return false;
            }
        }
        {
            let feed = self.feeds.entry(feed_id).or_insert_with(|| Feed {
                link: rss_link.to_owned(),
                title: rss.title.to_owned(),
                down_time: None,
                hash_list: rss.items.iter().map(gen_item_hash).collect(),
                subscribers: HashSet::new(),
            });
            feed.subscribers.insert(subscriber);
        }
        self.save().unwrap_or_default();
        true
    }

    pub fn unsubscribe(&mut self, subscriber: SubscriberID, rss_link: &str) -> Option<Feed> {
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

    pub fn delete_subscriber(&mut self, subscriber: SubscriberID) {
        self.subscribed_feeds(subscriber)
            .map(|feeds| {
                for feed in feeds {
                    let _ = self.unsubscribe(subscriber, &feed.link);
                }
            })
            .unwrap_or_default();
    }

    pub fn update_subscriber(&mut self, from: SubscriberID, to: SubscriberID) {
        let feeds = self.subscribers.remove(&from).unwrap();
        for feed_id in &feeds {
            let feed = self.feeds.get_mut(&feed_id).unwrap();
            feed.subscribers.remove(&from);
            feed.subscribers.insert(to);
        }
        self.subscribers.insert(to, feeds);
    }

    pub fn update(&mut self, rss_link: &str, items: Vec<feed::Item>) -> Vec<feed::Item> {
        let feed_id = gen_hash(&rss_link);
        if self.feeds.get(&feed_id).is_none() {
            return Vec::new();
        }

        self.reset_down_time(rss_link);

        let mut result = Vec::new();
        let mut new_hash_list = Vec::new();
        let items_len = items.len();
        for item in items {
            let hash = gen_item_hash(&item);
            if !self.feeds[&feed_id].hash_list.contains(&hash) {
                new_hash_list.push(hash);
                result.push(item);
            }
        }
        if !result.is_empty() {
            {
                let max_size = items_len * 2;
                let feed = self.feeds.get_mut(&feed_id).unwrap();
                let mut append: Vec<u64> = feed
                    .hash_list
                    .iter()
                    .take(max_size - new_hash_list.len())
                    .cloned()
                    .collect();
                new_hash_list.append(&mut append);
                feed.hash_list = new_hash_list;
            }
            self.save().unwrap_or_default();
        }
        result
    }

    pub fn update_title(&mut self, rss_link: &str, new_title: &str) {
        let feed_id = gen_hash(&rss_link);
        self.feeds
            .get_mut(&feed_id)
            .map(|feed| feed.title = new_title.to_owned())
            .unwrap_or_default();
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

fn gen_item_hash(item: &feed::Item) -> u64 {
    item.id.as_ref().map(|id| gen_hash(&id)).unwrap_or_else(|| {
        let title = item.title.as_ref().map(|s| s.as_str()).unwrap_or_default();
        let link = item.link.as_ref().map(|s| s.as_str()).unwrap_or_default();
        gen_hash(&format!("{}{}", title, link))
    })
}

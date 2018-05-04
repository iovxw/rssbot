use std;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::rc::Rc;

use serde_json;

use errors::*;
use feed;

fn get_hash<T: Hash>(t: &T) -> u64 {
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
    pub error_count: u32,
    pub subscribers: HashSet<SubscriberID>,
    hash_list: Vec<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Hub {
    pub callback: String,
    pub secret: String,
}

#[derive(Debug)]
struct DatabaseInner {
    path: String,
    feeds: HashMap<FeedID, Feed>,
    subscribers: HashMap<SubscriberID, HashSet<FeedID>>,
}

impl DatabaseInner {
    fn get_all_feeds(&self) -> Vec<Feed> {
        self.feeds.iter().map(|(_, v)| v.clone()).collect()
    }

    fn get_all_subscribers(&self) -> Vec<SubscriberID> {
        self.subscribers.iter().map(|(k, _)| *k).collect()
    }

    fn get_subscribed_feeds(&self, subscriber: SubscriberID) -> Option<Vec<Feed>> {
        self.subscribers.get(&subscriber).map(|feeds| {
            feeds
                .iter()
                .map(|feed_id| &self.feeds[feed_id])
                .cloned()
                .collect()
        })
    }

    fn inc_error_count(&mut self, rss_link: &str) -> u32 {
        let feed_id = get_hash(&rss_link);
        self.feeds
            .get_mut(&feed_id)
            .map(|feed| {
                feed.error_count += 1;
                feed.error_count
            })
            .unwrap_or_default()
    }

    fn reset_error_count(&mut self, rss_link: &str) {
        let feed_id = get_hash(&rss_link);
        self.feeds
            .get_mut(&feed_id)
            .map(|feed| feed.error_count = 0)
            .unwrap_or_default();
    }

    fn is_subscribed(&self, subscriber: SubscriberID, rss_link: &str) -> bool {
        self.subscribers
            .get(&subscriber)
            .map(|feeds| feeds.contains(&get_hash(&rss_link)))
            .unwrap_or(false)
    }

    fn subscribe(
        &mut self,
        subscriber: SubscriberID,
        rss_link: &str,
        rss: &feed::RSS,
    ) -> Result<()> {
        let feed_id = get_hash(&rss_link);
        {
            let subscribed_feeds = self.subscribers
                .entry(subscriber)
                .or_insert_with(HashSet::new);
            if !subscribed_feeds.insert(feed_id) {
                return Err(ErrorKind::AlreadySubscribed.into());
            }
        }
        {
            let feed = self.feeds.entry(feed_id).or_insert_with(|| Feed {
                link: rss_link.to_owned(),
                title: rss.title.to_owned(),
                error_count: 0,
                hash_list: rss.items.iter().map(gen_item_hash).collect(),
                subscribers: HashSet::new(),
            });
            feed.subscribers.insert(subscriber);
        }
        self.save()
    }

    fn unsubscribe(&mut self, subscriber: SubscriberID, rss_link: &str) -> Result<Feed> {
        let feed_id = get_hash(&rss_link);

        let clear_subscriber;
        if let Some(subscribed_feeds) = self.subscribers.get_mut(&subscriber) {
            if subscribed_feeds.remove(&feed_id) {
                clear_subscriber = subscribed_feeds.is_empty();
            } else {
                return Err(ErrorKind::NotSubscribed.into());
            }
        } else {
            return Err(ErrorKind::NotSubscribed.into());
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
                return Err(ErrorKind::NotSubscribed.into());
            }
        } else {
            return Err(ErrorKind::NotSubscribed.into());
        };
        if clear_feed {
            self.feeds.remove(&feed_id);
        }
        self.save()?;
        Ok(result)
    }

    fn delete_subscriber(&mut self, subscriber: SubscriberID) {
        self.get_subscribed_feeds(subscriber)
            .map(|feeds| {
                for feed in feeds {
                    let _ = self.unsubscribe(subscriber, &feed.link);
                }
            })
            .unwrap_or_default();
    }

    fn update_subscriber(&mut self, from: SubscriberID, to: SubscriberID) {
        let feeds = self.subscribers.remove(&from).unwrap();
        for feed_id in &feeds {
            let feed = self.feeds.get_mut(&feed_id).unwrap();
            feed.subscribers.remove(&from);
            feed.subscribers.insert(to);
        }
        self.subscribers.insert(to, feeds);
    }

    fn update(&mut self, rss_link: &str, items: Vec<feed::Item>) -> Vec<feed::Item> {
        let feed_id = get_hash(&rss_link);
        if self.feeds.get(&feed_id).is_none() {
            return Vec::new();
        }

        self.reset_error_count(rss_link);

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
                let mut append: Vec<u64> = feed.hash_list
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

    fn update_title(&mut self, rss_link: &str, new_title: &str) {
        let feed_id = get_hash(&rss_link);
        self.feeds
            .get_mut(&feed_id)
            .map(|feed| feed.title = new_title.to_owned())
            .unwrap_or_default();
    }

    fn save(&self) -> Result<()> {
        let feeds_list: Vec<&Feed> = self.feeds.iter().map(|(_id, feed)| feed).collect();
        let mut file =
            File::create(&self.path).chain_err(|| ErrorKind::DatabaseSave(self.path.to_owned()))?;
        serde_json::to_writer(&mut file, &feeds_list)
            .chain_err(|| ErrorKind::DatabaseSave(self.path.to_owned()))
    }
}

#[derive(Debug)]
pub struct Database {
    inner: Rc<RefCell<DatabaseInner>>,
}

impl Clone for Database {
    fn clone(&self) -> Database {
        Database {
            inner: Rc::clone(&self.inner),
        }
    }
}

fn gen_item_hash(item: &feed::Item) -> u64 {
    item.id.as_ref().map(|id| get_hash(&id)).unwrap_or_else(|| {
        let title = item.title.as_ref().map(|s| s.as_str()).unwrap_or_default();
        let link = item.link.as_ref().map(|s| s.as_str()).unwrap_or_default();
        get_hash(&format!("{}{}", title, link))
    })
}

impl Database {
    pub fn create(path: &str) -> Result<Database> {
        let feeds: HashMap<FeedID, Feed> = HashMap::new();
        let subscribers: HashMap<SubscriberID, HashSet<FeedID>> = HashMap::new();
        let result = Database {
            inner: Rc::new(RefCell::new(DatabaseInner {
                path: path.to_owned(),
                feeds: feeds,
                subscribers: subscribers,
            })),
        };

        result.save()?;

        Ok(result)
    }

    pub fn open(path: &str) -> Result<Database> {
        let p = Path::new(path);
        if p.exists() {
            let f = File::open(path).chain_err(|| ErrorKind::DatabaseOpen(path.to_owned()))?;
            let feeds_list: Vec<Feed> =
                serde_json::from_reader(&f).chain_err(|| ErrorKind::DatabaseFormat)?;

            let mut feeds: HashMap<FeedID, Feed> = HashMap::with_capacity(feeds_list.len());
            let mut subscribers: HashMap<SubscriberID, HashSet<FeedID>> = HashMap::new();

            for feed in feeds_list {
                let feed_id = get_hash(&feed.link);
                for subscriber in &feed.subscribers {
                    let subscribed_feeds = subscribers
                        .entry(subscriber.to_owned())
                        .or_insert_with(HashSet::new);
                    subscribed_feeds.insert(feed_id);
                }
                feeds.insert(feed_id, feed);
            }

            Ok(Database {
                inner: Rc::new(RefCell::new(DatabaseInner {
                    path: path.to_owned(),
                    feeds: feeds,
                    subscribers: subscribers,
                })),
            })
        } else {
            Database::create(path)
        }
    }

    pub fn get_all_feeds(&self) -> Vec<Feed> {
        self.inner.borrow().get_all_feeds()
    }

    pub fn get_all_subscribers(&self) -> Vec<SubscriberID> {
        self.inner.borrow().get_all_subscribers()
    }

    pub fn get_subscribed_feeds(&self, subscriber: SubscriberID) -> Option<Vec<Feed>> {
        self.inner.borrow().get_subscribed_feeds(subscriber)
    }

    pub fn inc_error_count(&self, rss_link: &str) -> u32 {
        self.inner.borrow_mut().inc_error_count(rss_link)
    }

    pub fn reset_error_count(&self, rss_link: &str) {
        self.inner.borrow_mut().reset_error_count(rss_link)
    }

    pub fn is_subscribed(&self, subscriber: SubscriberID, rss_link: &str) -> bool {
        self.inner.borrow().is_subscribed(subscriber, rss_link)
    }

    pub fn subscribe(
        &self,
        subscriber: SubscriberID,
        rss_link: &str,
        rss: &feed::RSS,
    ) -> Result<()> {
        self.inner.borrow_mut().subscribe(subscriber, rss_link, rss)
    }

    pub fn unsubscribe(&self, subscriber: SubscriberID, rss_link: &str) -> Result<Feed> {
        self.inner.borrow_mut().unsubscribe(subscriber, rss_link)
    }

    pub fn delete_subscriber(&self, subscriber: SubscriberID) {
        self.inner.borrow_mut().delete_subscriber(subscriber);
    }

    pub fn update_subscriber(&self, from: SubscriberID, to: SubscriberID) {
        self.inner.borrow_mut().update_subscriber(from, to);
    }

    pub fn update(&self, rss_link: &str, items: Vec<feed::Item>) -> Vec<feed::Item> {
        self.inner.borrow_mut().update(rss_link, items)
    }

    pub fn update_title(&self, rss_link: &str, new_title: &str) {
        self.inner.borrow_mut().update_title(rss_link, new_title)
    }

    fn save(&self) -> Result<()> {
        self.inner.borrow().save()
    }
}

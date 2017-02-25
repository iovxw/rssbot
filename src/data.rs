use std;
use std::fs::File;
use std::path::Path;
use std::hash::{Hash, Hasher};
use std::collections::{HashMap, HashSet};

use rss;
use serde_json;

use errors::*;

fn get_hash<T: Hash>(t: T) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::default();
    t.hash(&mut hasher);
    hasher.finish()
}

type FeedID = u64;
type SubscriberID = i64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feed {
    pub link: String,
    pub title: String,
    pub error_count: u32,
    pub subscribers: HashSet<SubscriberID>,
    hash_list: HashSet<u64>,
}

#[derive(Debug)]
pub struct Database {
    path: String,
    feeds: HashMap<FeedID, Feed>,
    subscribers: HashMap<SubscriberID, HashSet<FeedID>>,
}

fn gen_item_hash(item: &rss::Item) -> u64 {
    item.guid
        .as_ref()
        .map(|guid| get_hash(&guid.value))
        .unwrap_or_else(|| {
            let default = "".to_string();
            let title = item.title.as_ref().unwrap_or(&default);
            let link = item.link.as_ref().unwrap_or(&default);
            let string = String::with_capacity(title.len() + link.len());
            get_hash(string)
        })
}

fn truncate_hashset<T, S>(set: &mut HashSet<T, S>, u: usize)
    where T: std::hash::Hash + Clone + Eq,
          S: std::hash::BuildHasher
{
    let len = set.len();
    if len <= u {
        return;
    }
    let removed: Vec<T> = set.iter().take(u - len).map(|v| v.clone()).collect();
    for r in removed {
        set.remove(&r);
    }
}

impl Database {
    pub fn create(path: &str) -> Result<Database> {
        let feeds: HashMap<FeedID, Feed> = HashMap::new();
        let subscribers: HashMap<SubscriberID, HashSet<FeedID>> = HashMap::new();
        let mut result = Database {
            path: path.to_owned(),
            feeds: feeds,
            subscribers: subscribers,
        };

        result.save()?;

        Ok(result)
    }

    pub fn open(path: &str) -> Result<Database> {
        let p = Path::new(path);
        if p.exists() {
            let f = File::open(path).chain_err(|| ErrorKind::DatabaseOpen(path.to_owned()))?;
            let feeds_list: Vec<Feed> = serde_json::from_reader(&f).chain_err(|| ErrorKind::DatabaseFormat)?;

            let mut feeds: HashMap<FeedID, Feed> = HashMap::with_capacity(feeds_list.len());
            let mut subscribers: HashMap<SubscriberID, HashSet<FeedID>> = HashMap::new();

            for feed in feeds_list {
                let feed_id = get_hash(&feed.link);
                for subscriber in &feed.subscribers {
                    let subscribed_feeds = subscribers.entry(subscriber.to_owned())
                        .or_insert_with(|| HashSet::new());
                    subscribed_feeds.insert(feed_id);
                }
                feeds.insert(feed_id, feed);
            }

            Ok(Database {
                path: path.to_owned(),
                feeds: feeds,
                subscribers: subscribers,
            })
        } else {
            Database::create(path)
        }
    }

    pub fn get_all_feeds(&self) -> Vec<Feed> {
        self.feeds.iter().map(|(_, v)| v.clone()).collect()
    }

    pub fn get_subscribed_feeds(&self, subscriber: SubscriberID) -> Option<Vec<&Feed>> {
        self.subscribers.get(&subscriber).map(|feeds| {
            feeds.iter()
                .map(|feed_id| &self.feeds[feed_id])
                .collect()
        })
    }

    pub fn inc_error_count(&mut self, rss_link: &str) -> u32 {
        let feed_id = get_hash(rss_link);
        self.feeds
            .get_mut(&feed_id)
            .map(|feed| {
                feed.error_count += 1;
                feed.error_count
            })
            .unwrap_or_default()
    }

    pub fn is_subscribed(&self, subscriber: SubscriberID, rss_link: &str) -> bool {
        self.subscribers.get(&subscriber).map(|feeds| feeds.contains(&get_hash(rss_link))).unwrap_or(false)
    }

    pub fn subscribe(&mut self, subscriber: SubscriberID, rss_link: &str, rss: &rss::Channel) -> Result<()> {
        let feed_id = get_hash(rss_link);
        {
            let subscribed_feeds = self.subscribers.entry(subscriber).or_insert_with(|| HashSet::new());
            if !subscribed_feeds.insert(feed_id) {
                return Err(ErrorKind::AlreadySubscribed.into());
            }
        }
        {
            let feed = self.feeds.entry(feed_id).or_insert_with(|| {
                Feed {
                    link: rss_link.to_owned(),
                    title: rss.title.to_owned(),
                    error_count: 0,
                    hash_list: rss.items.iter().map(gen_item_hash).collect(),
                    subscribers: HashSet::new(),
                }
            });
            feed.subscribers.insert(subscriber);
        }
        self.save()
    }

    pub fn unsubscribe(&mut self, subscriber: SubscriberID, rss_link: &str) -> Result<Feed> {
        let feed_id = get_hash(rss_link);

        {
            let mut clear_subscriber = false;
            self.subscribers
                .get_mut(&subscriber)
                .map(|subscribed_feeds| if !subscribed_feeds.remove(&feed_id) {
                    Err::<(), Error>(ErrorKind::NotSubscribed.into())
                } else {
                    clear_subscriber = subscribed_feeds.len() == 0;
                    Ok(())
                })
                .unwrap_or(Err(ErrorKind::NotSubscribed.into()))?;
            if clear_subscriber {
                self.subscribers.remove(&subscriber);
            }
        }
        let result;
        {
            let mut clear_feed = false;
            result = self.feeds
                .get_mut(&feed_id)
                .map(|feed| if !feed.subscribers.remove(&subscriber) {
                    Err::<Feed, Error>(ErrorKind::NotSubscribed.into())
                } else {
                    clear_feed = feed.subscribers.len() == 0;
                    Ok(feed.clone())
                })
                .unwrap_or(Err(ErrorKind::NotSubscribed.into()))?;
            if clear_feed {
                self.feeds.remove(&feed_id);
            }
        }
        self.save()?;
        Ok(result)
    }

    pub fn update<'a>(&mut self, rss_link: &str, items: Vec<rss::Item>) -> Vec<rss::Item> {
        let feed_id = get_hash(rss_link);
        let mut result = Vec::new();
        let mut new_hash = Vec::new();
        let items_len = items.len();
        for item in items {
            let hash = gen_item_hash(&item);
            if !self.feeds[&feed_id].hash_list.contains(&hash) {
                result.push(item);
                new_hash.push(hash);
            }
        }
        if !new_hash.is_empty() {
            self.feeds.get_mut(&feed_id).map(|feed| {
                truncate_hashset(&mut feed.hash_list, items_len * 2 - new_hash.len());
                for hash in new_hash {
                    feed.hash_list.insert(hash);
                }
            });
        }
        self.save().unwrap_or_default();
        result
    }

    fn save(&mut self) -> Result<()> {
        let feeds_list: Vec<&Feed> = self.feeds.iter().map(|(_id, feed)| feed).collect();
        let mut file = File::create(&self.path).chain_err(|| ErrorKind::DatabaseSave(self.path.to_owned()))?;
        serde_json::to_writer(&mut file, &feeds_list).chain_err(|| ErrorKind::DatabaseSave(self.path.to_owned()))
    }
}

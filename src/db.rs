use anyhow::{bail, Result};
use chrono::{serde::ts_milliseconds, DateTime, Utc};
use mail_parser::{HeaderValue, Message};
use mongodb::Collection;
use rss::{GuidBuilder, Item, ItemBuilder};
use serde::{Deserialize, Serialize};
use tracing::{info, info_span, warn, Instrument};

use crate::{config::get_config, RX};

pub type Feeds = Collection<Feed>;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Feed {
    pub id: String,
    #[serde(with = "ts_milliseconds")]
    pub created_at: DateTime<Utc>,
    pub title: String,
    pub author: String,
    pub content: String,
    pub raw: String,
    pub from_box: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Index {
    pub id: String,
}

impl Feed {
    pub fn into_rss(self) -> Item {
        let config = get_config();

        let guid = GuidBuilder::default()
            .permalink(true)
            .value(format!("{}", self.id))
            .build();

        ItemBuilder::default()
            .title(self.title)
            .link(Some(format!(
                "https://{}/feeds/{}",
                config.web_domain, self.id
            )))
            .author(Some(self.author))
            .pub_date(Some(self.created_at.to_rfc2822()))
            .guid(Some(guid))
            .content(Some(self.content))
            .build()
    }

    pub fn trace(&self) {
        let Self {
            id,
            title,
            author,
            content,
            ..
        } = self;
        info!(
            id = id.as_str(),
            title = title.as_str(),
            author = author.as_str(),
            len = content.len(),
            "New Feed",
        )
    }
}

pub trait ToVec {
    fn to_vec(&self) -> Vec<String>;
}

impl<'a> ToVec for mail_parser::Addr<'a> {
    fn to_vec(&self) -> Vec<String> {
        self.address
            .as_ref()
            .map(|x| vec![x.to_string()])
            .unwrap_or_default()
    }
}

impl<'a> ToVec for Vec<mail_parser::Addr<'a>> {
    fn to_vec(&self) -> Vec<String> {
        self.iter().flat_map(|x| x.to_vec()).collect()
    }
}

impl<'a> ToVec for mail_parser::Group<'a> {
    fn to_vec(&self) -> Vec<String> {
        self.addresses.to_vec()
    }
}

impl<'a> ToVec for Vec<mail_parser::Group<'a>> {
    fn to_vec(&self) -> Vec<String> {
        self.iter().flat_map(|x| x.to_vec()).collect()
    }
}

impl<'a> ToVec for HeaderValue<'a> {
    fn to_vec(&self) -> Vec<String> {
        match self {
            HeaderValue::Address(addr) => addr.to_vec(),
            HeaderValue::AddressList(list) => list.to_vec(),
            HeaderValue::Group(group) => group.to_vec(),
            HeaderValue::GroupList(list) => list.to_vec(),
            HeaderValue::Text(content) => vec![content.to_string()],
            HeaderValue::TextList(list) => list.iter().map(|x| x.to_string()).collect(),
            _ => vec![],
        }
    }
}

impl<'a> TryFrom<(&'a Vec<u8>, Message<'a>)> for Feed {
    type Error = anyhow::Error;
    fn try_from((raw, val): (&'a Vec<u8>, Message<'a>)) -> Result<Self> {
        let config = get_config();
        let from_box = match get_box(&val) {
            Some(x) => x,
            None => bail!("Not sending to {}, blocked", config.domain),
        };
        let author = match val.get_from() {
            HeaderValue::Address(addr) => match (addr.address.as_ref(), addr.name.as_ref()) {
                (Some(addr), Some(name)) => format!("{} ({})", addr, name),
                (None, Some(name)) => name.to_string(),
                (Some(addr), None) => addr.to_string(),
                _ => "Unknown".to_owned(),
            },
            _ => "Unknown".to_owned(),
        };
        let title = val.get_subject().unwrap_or("Unknown Title").to_owned();
        let created_at = Utc::now();
        let content = val
            .get_html_bodies()
            .flat_map(|x| x.get_contents().to_vec())
            .collect::<Vec<_>>();
        Ok(Feed {
            raw: String::from_utf8(raw.to_owned())?,
            content: String::from_utf8(content)?,
            created_at,
            title,
            author,
            from_box,
            id: nanoid::nanoid!(10),
        })
    }
}

pub async fn database_servo(collection: Feeds, rx: RX) {
    info!(target: "Database", "Starting");

    while let Ok(feed) = rx.recv().await {
        let span = info_span!("Database.insert");
        feed.trace();
        if let Err(e) = collection.insert_one(feed, None).instrument(span).await {
            warn!(target: "Database", "Error insert doc: {}", e)
        }
    }

    info!(target: "Database", "Stopping");
}

fn get_box(val: &Message) -> Option<String> {
    let config = get_config();
    let mut receivers = val.get_to().to_vec();
    receivers.sort();

    // Check "To" header against the domain
    let domain_suffix = format!("@{}", config.domain);
    let ret = receivers
        .iter()
        .filter(|x| x.contains(&domain_suffix))
        .next();
    if ret.is_some() {
        return Some(ret.unwrap().to_owned());
    }

    // Check the rules
    let rules = &config.rules;
    return rules
        .iter()
        .filter(|rule| {
            rule.filter
                .iter()
                .filter(|fl| fl.matches(&val))
                .next()
                .is_some()
        })
        .map(|x| x.to_box.to_owned())
        .next();
}

#[derive(Deserialize, Serialize)]
pub struct Summary {
    pub title: String,
    pub create_at: String,
    pub id: String,
}
#[derive(Deserialize, Serialize)]
pub struct List {
    pub items: Vec<Summary>,
}

#[test]
fn test() {
    const RAW: &str = include_str!("../sample.txt");
    let parsed = mail_parser::Message::parse(RAW.as_bytes()).unwrap();
    println!("{:#?}", parsed);
}

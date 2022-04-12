use std::{env::var, fs};

use anyhow::Result;
use once_cell::sync::Lazy;
use serde_json::from_str;
use tracing::warn;

use crate::rule::{Rule, RuleFilter};

static CONFIG: Lazy<Config> = Lazy::new(|| Config::from_env().unwrap());

#[derive(Clone, Debug)]
pub struct Config {
    pub web_port: u16,
    pub smtp_port: u16,
    pub per_page: u16,
    pub domain: String,
    pub mongo_con_str: String,
    pub mongo_db_name: String,
    pub web_domain: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub rules: Vec<Rule>,
    pub disable_rcpt_filter: bool,
    pub default_page_limit: i64,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let rules = match var("RULE_FILE") {
            Ok(path) => match fs::read_to_string(path) {
                Ok(text) => match from_str::<Vec<Rule>>(&text) {
                    Ok(rules) => rules,
                    Err(e) => {
                        warn!("Error parsing rules: {}", e);
                        vec![]
                    }
                },
                Err(e) => {
                    warn!("Error parsing rules: {}", e);
                    vec![]
                }
            },
            Err(_e) => vec![],
        };
        let domain = var("DOMAIN").unwrap_or_else(|_| "example.com".to_owned());
        let ret = Self {
            web_port: var("WEB_PORT").map_or_else(|_| Ok(8080), |x| x.parse())?,
            smtp_port: var("SMTP_PORT").map_or_else(|_| Ok(10000), |x| x.parse())?,
            per_page: var("PER_PAGE").map_or_else(|_| Ok(10), |x| x.parse())?,
            domain: domain.clone(),
            mongo_con_str: var("MONGO_CON_STR")
                .unwrap_or_else(|_| "mongodb://localhost:27017".to_owned()),
            mongo_db_name: var("MONGO_DB_NAME").unwrap_or_else(|_| "mail-list-rss".to_owned()),
            web_domain: var("WEB_DOMAIN").map_or(domain.clone(), |x| {
                if x.is_empty() {
                    domain.clone()
                } else {
                    x
                }
            }),
            username: var("AUTH_USERNAME").ok(),
            password: var("AUTH_PASSWORD").ok(),
            disable_rcpt_filter: rules
                .iter()
                .filter(|rule| {
                    rule.filter
                        .iter()
                        .filter(|fltr| matches!(fltr, RuleFilter::ByFrom(_)))
                        .next()
                        .is_some()
                })
                .next()
                .is_some(),
            rules,
            default_page_limit: var("DEFAULT_PAGE_LIMIT").map_or_else(|_| Ok(30), |x| x.parse())?,
        };

        if ret.username.is_some() ^ ret.password.is_some() {
            // Only one exist and the other is not set
            panic!("Both username and password should be set or not set");
        }

        Ok(ret)
    }
}

#[inline]
pub fn get_config<'a>() -> &'a Config {
    &CONFIG
}

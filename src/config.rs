use std::env::var;

use anyhow::Result;
use once_cell::sync::Lazy;

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
}

impl Config {
    pub fn from_env() -> Result<Self> {
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

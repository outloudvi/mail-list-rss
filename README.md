# mail-list-rss

Used to convert newsletter subscription sent with SMTP into RSS feeds. Use mongodb for storing received contents.
Does not support TLS in both web and SMTP so reverse proxy will be needed for production usage.

## Getting startted 

Configuation is purely passed in with environment variables.
Also mail-list-rss comes with a sane default setting for development usage. But in production usage, it is highly recommended to tune the configs 
to fit your own usage, especially two prefixed with `AUTH_`.  Here is a list of variables. 

- `WEB_PORT`
- `SMTP_PORT`
- `PER_PAGE`
- `DOMAIN`
- `MONGO_CON_STR`
- `MONGO_DB_NAME`
- `AUTH_USERNAME`
- `AUTH_PASSWORD`

**Note**: `AUTH_USERNAME` and `AUTH_PASSWORD` should be used in pair.

For more details see [ronfig.rs](./blob/master/src/config.rs)

### Docker

You can use docker to deploy and run. Don't forget to expose web and smtp port.

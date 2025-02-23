use std::{collections::HashMap, net::SocketAddr, str::FromStr};

use anyhow::Result;
use axum::{
    extract::{Extension, Path, Query},
    handler::Handler,
    http::{
        header::{self, HeaderName, CONTENT_TYPE},
        uri::{Authority, Scheme},
        HeaderValue, Request, StatusCode,
    },
    response::{Headers, Html, IntoResponse, Redirect, Response},
    routing::{any, get},
    AddExtensionLayer, Json, Router,
};
use axum_extra::middleware::{middleware_fn, Next};
use chrono::Utc;
use futures::{StreamExt, TryStreamExt};
use mongodb::{
    bson::{doc, Document},
    options::{DistinctOptions, FindOptions},
};
use serde::Deserialize;
use tower_http::{
    auth::RequireAuthorizationLayer,
    cors,
    set_header::SetResponseHeaderLayer,
    trace::{OnRequest, OnResponse, TraceLayer},
};
use tracing::{info, log::warn, Level};

use crate::{
    config::get_config,
    db::{Feeds, List, Summary},
};

fn utf8_header(res: &Response) -> Option<HeaderValue> {
    if let Some(header) = res.headers().get(CONTENT_TYPE) {
        if let Ok(header) = header.to_str() {
            if !header.ends_with("charset=utf-8") {
                let mut ret = header.to_owned();
                ret.push_str("; charset=utf-8");
                return HeaderValue::from_str(&ret).ok();
            }
        }
    }
    None
}

async fn http_rediretor<B>(req: Request<B>, next: Next<B>) -> impl IntoResponse {
    let config = get_config();

    match req
        .headers()
        .get(HeaderName::from_lowercase(b"x-forwarded-proto").unwrap())
    {
        Some(schema) if schema.to_str().map(|x| x != "https").unwrap_or(true) => {
            let mut parts = req.uri().clone().into_parts();
            parts.scheme = Some(Scheme::HTTPS);
            parts.authority = Some(Authority::from_str(&config.web_domain).expect("Bad domain"));
            Err(Redirect::permanent(parts.try_into().unwrap()))
        }
        _ => Ok(next.run(req).await),
    }
}

#[derive(Copy, Clone)]
struct Logger {}

impl<B> OnRequest<B> for Logger {
    fn on_request(&mut self, request: &axum::http::Request<B>, _: &tracing::Span) {
        let route = request.uri().path();
        tracing::event!(target: "web", Level::INFO, route);
    }
}

impl<B> OnResponse<B> for Logger {
    fn on_response(
        self,
        response: &axum::http::Response<B>,
        latency: std::time::Duration,
        _: &tracing::Span,
    ) {
        let status = response.status().as_u16();
        let time = latency.as_secs_f32();
        tracing::event!(target: "web", Level::INFO, status, time)
    }
}

pub async fn web_server(collection: Feeds) -> Result<()> {
    let logger = Logger {};

    let utf8_layer = SetResponseHeaderLayer::overriding(CONTENT_TYPE, utf8_header);
    let config = get_config();

    let mut app = Router::new()
        .route("/", get(index))
        .route("/feeds/:key", get(rendered_html))
        .route("/feeds/:key/raw", get(raw))
        .route("/feeds", get(list.layer(utf8_layer)))
        .route("/rss", get(rss))
        .route("/rss/:box", get(rss_box))
        .route("/boxes", get(boxes))
        .layer(AddExtensionLayer::new(collection))
        .layer(
            TraceLayer::new_for_http()
                .on_request(logger)
                .on_response(logger),
        );

    if config.username.is_some() {
        info!(
            target: "web",
            "Using basic auth"
        );
        app = app.layer(RequireAuthorizationLayer::basic(
            config.username.as_ref().unwrap(),
            config.password.as_ref().unwrap(),
        ))
    } else {
        warn!(target: "web", "No auth configured, this can be dangerous and should only be used in development");
    }

    app = app
        .route("/health", any(|| async { "OK" }))
        .route_layer(middleware_fn::from_fn(http_rediretor))
        .route_layer(
            cors::CorsLayer::new()
                .allow_headers(cors::any())
                .allow_methods(cors::any())
                .allow_origin(cors::any()),
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], config.web_port));

    info!(target: "web", "Starting");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();

    info!(target: "web", "Stopped");

    Ok(())
}

async fn index() -> impl IntoResponse {
    Html(include_str!("../front/dist/index.html"))
}

async fn rss(Extension(feed): Extension<Feeds>) -> impl IntoResponse {
    let config = get_config();
    match render_feeds(feed, None, &format!("https://{}/rss", config.web_domain)).await {
        Ok(content) => (
            StatusCode::OK,
            Headers(vec![(
                header::CONTENT_TYPE,
                "application/xml; charset=utf-8",
            )]),
            content,
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Headers(vec![]),
            e.to_string(),
        ),
    }
}

async fn rss_box(
    Path(map): Path<HashMap<String, String>>,
    Extension(feed): Extension<Feeds>,
) -> impl IntoResponse {
    let config = get_config();
    let email = map.get("box").expect("box name should exist");
    match render_feeds(
        feed,
        Some(doc! { "from_box": email }),
        &format!("https://{}/rss/{}", config.web_domain, email),
    )
    .await
    {
        Ok(content) => (
            StatusCode::OK,
            Headers(vec![(
                header::CONTENT_TYPE,
                "application/xml; charset=utf-8",
            )]),
            content,
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Headers(vec![]),
            e.to_string(),
        ),
    }
}

async fn render_feeds(feeds: Feeds, filter: Option<Document>, link: &str) -> Result<String> {
    let config = get_config();
    let option = FindOptions::builder()
        .limit(config.per_page as i64)
        .sort(doc! { "created_at": -1 })
        .build();
    let feeds = feeds
        .find(filter, option)
        .await?
        .try_fold(Vec::with_capacity(10), |mut acc, x| async move {
            acc.push(x.into_rss());
            Ok(acc)
        })
        .await?;

    let ret = rss::ChannelBuilder::default()
        .title("Mail List")
        .generator(Some("http://github.com/George-Miao/mail-list-rss".into()))
        .link(link)
        .pub_date(Utc::now().to_rfc2822())
        .items(feeds)
        .build()
        .to_string();
    Ok(ret)
}

#[derive(Deserialize)]
struct FeedsQuery {
    limit: Option<i64>,
    skip: Option<u64>,
}

async fn list(Extension(feeds): Extension<Feeds>, query: Query<FeedsQuery>) -> impl IntoResponse {
    Json(render_list(feeds, query.limit, query.skip).await.unwrap())
}

async fn render_list(feeds: Feeds, limit: Option<i64>, skip: Option<u64>) -> Result<List> {
    let config = get_config();
    let res = feeds
        .find(
            None,
            FindOptions::builder()
                .limit(limit.unwrap_or(config.default_page_limit))
                .skip(skip)
                .sort(doc! { "created_at": -1 })
                .build(),
        )
        .await?
        .filter_map(|x| async move {
            x.ok().map(|x| Summary {
                create_at: x.created_at.to_rfc2822(),
                title: x.title,
                id: x.id,
            })
        })
        .collect::<Vec<_>>()
        .await;

    Ok(List { items: res })
}

async fn rendered_html(
    Path(map): Path<HashMap<String, String>>,
    Extension(feeds): Extension<Feeds>,
) -> impl IntoResponse {
    let key = map.get("key").expect("key should exist");
    let res = feeds.find_one(doc! { "id" : key }, None).await;
    match res {
        Ok(Some(res)) => (
            StatusCode::OK,
            Headers(vec![(header::CONTENT_TYPE, "text/html; charset=utf-8")]),
            res.content,
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Headers(vec![]),
            format!("Cannot find {}", key),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Headers(vec![]),
            e.to_string(),
        ),
    }
}

async fn raw(
    Path(map): Path<HashMap<String, String>>,
    Extension(feeds): Extension<Feeds>,
) -> impl IntoResponse {
    let key = map.get("key").expect("key should exist");
    let res = feeds.find_one(doc! { "id" : key }, None).await;
    match res {
        Ok(Some(res)) => (
            StatusCode::OK,
            Headers(vec![(header::CONTENT_TYPE, "text/plain; charset=utf-8")]),
            res.raw,
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Headers(vec![]),
            format!("Cannot find {}", key),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Headers(vec![]),
            e.to_string(),
        ),
    }
}

async fn boxes(Extension(feed): Extension<Feeds>) -> impl IntoResponse {
    let option = DistinctOptions::builder().build();
    let emails = feed.distinct("from_box", None, option).await;
    match emails {
        Ok(content) => {
            let emails_text = content
                .iter()
                .map(|f| f.as_str().unwrap())
                .collect::<Vec<_>>();
            (
                StatusCode::OK,
                Headers(vec![(
                    header::CONTENT_TYPE,
                    "application/json; charset=utf-8",
                )]),
                serde_json::to_string(&emails_text).unwrap(),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Headers(vec![]),
            e.to_string(),
        ),
    }
}

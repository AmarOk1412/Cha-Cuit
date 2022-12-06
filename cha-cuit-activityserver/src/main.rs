use actix_web::{web, App, HttpServer};
use serde::Deserialize;

#[derive(Deserialize)]
struct InboxRequest {
    #[serde(rename = "type")]
    activity_type: String,
    id: String,
    actor: String,
    object: Object,
}

#[derive(Deserialize)]
struct OutboxRequest {
    #[serde(rename = "type")]
    activity_type: String,
    id: String,
    actor: String,
    object: Object,
}

#[derive(Deserialize)]
struct Object {
    #[serde(rename = "type")]
    object_type: String,
    name: String,
    content: String,
    attributedTo: String,
    mediaType: String,
}

async fn inbox(data: web::Json<InboxRequest>) -> &'static str {
    if data.activity_type == "Create" && data.object.object_type == "Article" {
        println!("Received article '{}' from {}", data.object.name, data.actor);
        "Article received"
    } else {
        "Activity received"
    }
}

async fn outbox(data: web::Json<OutboxRequest>) -> &'static str {
    if data.activity_type == "Create" && data.object.object_type == "Article" {
        println!("Posting article '{}' from {}", data.object.name, data.actor);
        "Article posted"
    } else {
        "Activity posted"
    }
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .route("/inbox", web::post().to(inbox))
            .route("/outbox", web::post().to(outbox))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}

// curl -XPOST -H "Content-Type: application/json" -d '{ "type": "Create", "id": "http://example.com/activities/1", "actor": "http://example.com/users/alice", "object": { "type": "Article", "name": "What a Crazy Day I Had", "content": "<div>... you will never believe ...</div>", "attributedTo": "http://sally.example.org" } }' http://127.0.0.1:8080/inbox
// TODO optional
// TODO only filter recipes
// TODO outbox from hugo
// TODO store recipes in fediverse
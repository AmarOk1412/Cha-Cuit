use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use serde_json::json;
use regex::Regex;
use std::fs;

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

#[derive(Deserialize, Serialize)]
struct Object {
    #[serde(rename = "type")]
    object_type: String,
    name: String,
    content: String,
    attributedTo: String,
    mediaType: String,
}

// Define the regular expression for matching the required header
// TODO order or generate all?
const REQUIRED_HEADER: &str = r#"^---\ntitle: ([^\n]+)\n?duration: ([^\n]+)\n?tags: (\[[^\]]+\])\n?thumbnail: ("[^"]+")\n?---\n"#;
const HEADER_TITLE_REGEX: &str = r#"title: ([^\n]+)\n"#;
// Define the regular expression for matching the required titles
const REQUIRED_TITLES: &str = r"# Ingrédients[\s\S]*# Équipement[\s\S]*# Préparation[\s\S]*# Notes";

async fn inbox(data: web::Json<InboxRequest>) -> &'static str {
    if data.activity_type == "Create" && data.object.object_type == "Article" && data.object.mediaType == "text/markdown" {
        // Compile the regular expression
        let re_header = Regex::new(REQUIRED_HEADER).unwrap();
        let re_titles = Regex::new(REQUIRED_TITLES).unwrap();

        // Check if the content of the article matches the required titles
        if re_header.is_match(&data.object.content) && re_titles.is_match(&data.object.content) {
            // Get the current date
            let now: DateTime<Local> = Local::now();
            let current_date = now.format("%Y-%m-%d").to_string();

            // Add the current date to the header of the article
            let new_content = re_header.replace(
                                                &data.object.content,
                                                format!("---\ntitle: {}\nduration: $2\ntags: $3\nthumbnail: $4\ndate: {}\n---\n", data.object.name, current_date)
                                            ).to_string();

            // Save the content of the article to a file
            let file_path = format!("chadiverse/{}.md", data.object.name);
            // Create the "chadiverse" directory if it does not exist
            if let Err(_) = fs::create_dir_all("chadiverse") {
                return "Failed to create directory";
            }

            // Write the file
            if let Err(_) = fs::write(file_path, new_content) {
                return "Failed to write file";
            }

            // TODO add tests
            println!("Received article '{}' from {}", data.object.name, data.actor);
            "Article received"
        } else {
            "Article does not have the required titles"
        }
    } else {
        "Activity received"
    }
}


async fn outbox() -> impl Responder {

    let mut articles = Vec::new();
    let markdown_files = fs::read_dir("../content/recettes/").unwrap();
    let re_title_regex = Regex::new(HEADER_TITLE_REGEX).unwrap();
    // Parse the markdown files into a collection of `Object`s.
    for markdown_file in markdown_files {
        let markdown = fs::read_to_string(markdown_file.unwrap().path()).unwrap();
        let match_title = re_title_regex.captures(&markdown).unwrap();
        let article = Object {
            object_type: "Article".to_owned(),
            name: match_title.get(1).map_or("ERR", |m| m.as_str()).to_owned(),
            content: markdown,
            attributedTo: "chef@cha-cu.it".to_owned(), // TODO
            mediaType: "text/markdown".to_owned(),
        };
        articles.push(article);
    }


    let outbox_json = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": "https://cha-cu.it/users/chef/outbox",
        "type": "OrderedCollection",
        "totalItems": articles.len(),
        "orderedItems": articles,
    });
    HttpResponse::Ok().json(outbox_json)
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .route("/inbox", web::post().to(inbox))
            .route("/users/chef/outbox", web::get().to(outbox))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
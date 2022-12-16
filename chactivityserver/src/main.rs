use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use chrono::{DateTime, Local};
use chrono::offset::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use regex::Regex;
use std::fs;
use std::io;
use std::time::SystemTime;
use std::time::Duration;

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


#[derive(Deserialize)]
pub struct OutboxParams {
    page: Option<u32>
}

async fn outbox(info: web::Query<OutboxParams>) -> impl Responder {
    let recipes = fs::read_dir("../content/recettes/").unwrap()
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<Vec<_>, io::Error>>().unwrap();
    if info.page.unwrap_or(0) == 0 {
        // If no page provided, then describe the other pages
        let outbox_json = json!({
            "@context": "https://www.w3.org/ns/activitystreams",
            "id": "https://cha-cu.it/users/chef/outbox",
            "type": "OrderedCollection",
            "totalItems": recipes.len(),
            "first": "https://cha-cu.it/users/chef/outbox?page=1",
            "last": format!("https://cha-cu.it/users/chef/outbox?page={}", (recipes.len()/12)+1),
        });
        return HttpResponse::Ok().json(outbox_json);
    }

    // sort the recipes by last modification time
    let mut sorted_recipes = recipes.iter().map(|entry| {
        let metadata = entry.metadata()?;
        let modified_time = metadata.modified()?;
        Ok((entry, modified_time))
    }).collect::<Result<Vec<_>, io::Error>>().unwrap();
    sorted_recipes.sort_by(|a, b| b.1.cmp(&a.1));

    // Create the "chadiverse" directory if it does not exist
    if let Err(_) = fs::create_dir_all(".cache") {
        println!("Failed to create directory");
        return HttpResponse::Ok().json(json!({}));
    }

    // read the date from the file
    let file_date_string = match fs::read_to_string(".cache/date_file.txt") {
        Ok(date_string) => date_string,
        Err(_) => "".to_string(), // if the file does not exist or there was an error reading it, use an empty string
    };

    // parse the date string from the file
    let file_date = match file_date_string.parse::<u64>() {
        Ok(date) =>  SystemTime::UNIX_EPOCH + Duration::from_secs(date),
        Err(_) => SystemTime::UNIX_EPOCH, // if the date string is invalid, use the Unix epoch as the date
    };

    // get the date of the first entry
    let first_entry_date = sorted_recipes[0].1;
    if first_entry_date > file_date {
        println!("TODO CACHE!");
    }

    let datetime: DateTime<Utc> = first_entry_date.into();
    println!("{:?} {}", datetime.format("%y%m%d"), sorted_recipes.len());



    let mut articles = Vec::new();
    let re_title_regex = Regex::new(HEADER_TITLE_REGEX).unwrap();
    // Parse the markdown files into a collection of `Object`s.
    for markdown_file in recipes {
        let markdown = fs::read_to_string(markdown_file).unwrap();
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

    let outbox_json = json!({});
    HttpResponse::Ok().json(outbox_json)
}


async fn profile() -> impl Responder {
    let profile_json = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "type": "Person",
        "id": "https://cha-cu.it/users/chef/",
        "name": "Chef",
        "preferredUsername": "chef",
        "summary": "Lisp enthusiast hailing from MIT",
        "inbox": "https://cha-cu.it/users/chef/inbox/",
        "outbox": "https://cha-cu.it/users/chef/outbox/",
    });
    HttpResponse::Ok().json(profile_json)
}

#[derive(Debug, Deserialize)]
pub struct WebFingerRequest {
    resource: String
}

async fn webfinger_handler(info: web::Query<WebFingerRequest>) -> impl Responder {
    if info.resource == "acct:chef@cha-cu.it" {
        return HttpResponse::Ok().json(json!({
            "subject" : info.resource,
            "links": [
                {
                    "rel":"http://webfinger.net/rel/profile-page","type":"text/html",
                    "href":"https://cha-cu.it/recettes"
                },
                {
                    "rel": "self",
                    "type": "application/activity+json",
                    "href": "https://cha-cu.it/users/chef"
                }
            ]
        }));
    }
    HttpResponse::Ok().json(json!({}))
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .route("/inbox", web::post().to(inbox))
            .route("/users/chef/outbox", web::get().to(outbox))
            .route("/users/chef", web::get().to(profile))
            .route("/.well-known/webfinger", web::get().to(webfinger_handler))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
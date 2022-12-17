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
    #[serde(rename = "@context")]
    context: String,
    id: String,
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

// TODO split
async fn outbox(info: web::Query<OutboxParams>) -> impl Responder {
    let recipes = fs::read_dir("../content/recettes/").unwrap()
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<Vec<_>, io::Error>>().unwrap();
    let page: usize = info.page.unwrap_or(0) as usize;
    let max_page: usize = (recipes.len()/12)+1;
    if page == 0 {
        // If no page provided, then describe the other pages
        let outbox_json = json!({
            "@context": "https://www.w3.org/ns/activitystreams",
            "id": "https://cha-cu.it/users/chef/outbox",
            "type": "OrderedCollection",
            "totalItems": recipes.len(),
            "first": "https://cha-cu.it/users/chef/outbox?page=1",
            "last": format!("https://cha-cu.it/users/chef/outbox?page={}", max_page),
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
    let contents = match fs::read_to_string(".cache/date_file.txt") {
        Ok(contents) => contents,
        Err(_) => "".to_string(), // if the file does not exist or there was an error reading it, use an empty string
    };

    let mut file_date = 0u64;
    let mut file_nb_articles = 0 as usize;
    let lines: Vec<&str> = contents.lines().collect();
    if lines.len() == 2 {
        // parse the date string from the file
        file_date = lines[0].parse::<u64>().unwrap_or(0);
        file_nb_articles = lines[1].parse().unwrap_or(0 as usize);
    }

    if let Err(_) = fs::create_dir_all("chadiverse") {
        println!("Failed to create directory");
        return HttpResponse::Ok().json(json!({}));
    }

    // get the date of the first entry
    let first_entry_date = sorted_recipes[0].1.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
    let chunked_recipes: Vec<Vec<_>> = sorted_recipes.chunks(12).map(|chunk| chunk.to_vec()).collect();
    if first_entry_date > file_date || file_nb_articles != recipes.len() {
        println!("@@@ CACHE {} {} {} {}", first_entry_date, file_date, file_nb_articles, recipes.len());
        let mut idx_page = 0;
        let re_title_regex = Regex::new(HEADER_TITLE_REGEX).unwrap();
        for chunk in chunked_recipes {
            let mut articles = Vec::new();
            // Parse the markdown files into a collection of `Object`s.
            for markdown_file in chunk.iter() {
                let filename_without_extension = markdown_file.0.file_stem().unwrap().to_str().unwrap();
                let markdown = fs::read_to_string(markdown_file.0).unwrap();
                let match_title = re_title_regex.captures(&markdown).unwrap();
                let article = Object {
                    context: "https://www.w3.org/ns/activitystreams".to_owned(),
                    id: format!("https://cha-cu.it/recettes/{}", filename_without_extension).to_owned(),
                    object_type: "Article".to_owned(),
                    name: match_title.get(1).map_or("ERR", |m| m.as_str()).to_owned(),
                    content: markdown,
                    attributedTo: "chef@cha-cu.it".to_owned(), // TODO
                    mediaType: "text/markdown".to_owned(),
                };
                articles.push(article);
            }

            let mut outbox_json = json!({
                "type": "OrderedCollectionPage",
                "partOf": "https://cha-cu.it/users/chef/outbox",
                "@context": "https://www.w3.org/ns/activitystreams",
                "items": articles
            });
            if idx_page > 1 {
                outbox_json["prev"] = serde_json::Value::String(format!("https://cha-cu.it/users/chef/outbox?page={}", idx_page - 1));
            }
            if idx_page < max_page {
                outbox_json["next"] = serde_json::Value::String(format!("https://cha-cu.it/users/chef/outbox?page={}", idx_page + 1));
            }

            // Cache file
            std::fs::write(
                format!(".cache/{}.json", idx_page + 1),
                serde_json::to_string_pretty(&outbox_json).unwrap(),
            ).unwrap();
            idx_page += 1;
        }


        let cache_content = format!("{}\n{}\n", first_entry_date, recipes.len());
        fs::write(".cache/date_file.txt", cache_content).unwrap();
    }

    let content = match fs::read_to_string(format!(".cache/{}.json", page)) {
        Ok(c) => c,
        Err(_) => "{}".to_owned(),
    };

    HttpResponse::Ok().json(serde_json::from_str::<serde_json::Value>(&content).unwrap())
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
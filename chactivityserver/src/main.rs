use actix_web::{web, App, HttpResponse, HttpServer, Responder, web::{Bytes, post}};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use serde_json::json;
use regex::Regex;
use std::fs;
use http_sig::*;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::prelude::*;
use std::io;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use serde_json::Value;

// TODO split in files
// TODO re-organize headers
// TODO license
// TODO option parameters in InboxRequest

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
    #[serde(rename = "type", default)]
    object_type: String,
    #[serde(rename = "@context", default)]
    context: String,
    #[serde(default)]
    actor: String,
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    content: String,
    #[serde(rename = "attributedTo", default)]
    attributed_to: String,
    #[serde(rename = "mediaType", default)]
    media_type: String,
}

#[derive(Deserialize, Serialize)]
struct ActivityPubRequest {
    #[serde(rename = "type", default)]
    object_type: String,
    #[serde(rename = "@context", default)]
    context: String,
}

#[derive(Deserialize, Serialize)]
struct FollowObject {
    #[serde(default)]
    id: String,
    #[serde(default)]
    actor: String,
    #[serde(default)]
    object: String,
}


// Define the regular expression for matching the required header
// TODO order or generate all?
const REQUIRED_HEADER: &str = r#"^---\ntitle: ([^\n]+)\n?duration: ([^\n]+)\n?tags: (\[[^\]]+\])\n?thumbnail: ("[^"]+")\n?---\n"#;
const HEADER_TITLE_REGEX: &str = r#"title: ([^\n]+)\n"#;
// Define the regular expression for matching the required titles
const REQUIRED_TITLES: &str = r"# Ingrédients[\s\S]*# Équipement[\s\S]*# Préparation[\s\S]*# Notes";

fn followers() -> Vec<String> {
    let mut followers : Vec<String> = Vec::new();
    if Path::new("chadiverse/followers").exists() {
        let file = File::open("chadiverse/followers").unwrap();
        let buf = BufReader::new(file);
        followers = buf.lines()
            .map(|l| l.expect("Could not parse line"))
            .collect();
    }
    followers
}

fn write_followers(followers: &Vec<String>) {
    // TODO serialize and refresh instead removing
    fs::remove_file("chadiverse/followers");
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .append(false)
        .open("chadiverse/followers")
        .unwrap();
    for follower in followers  {
        if let Err(e) = writeln!(file, "{}", follower) {
            eprintln!("Couldn't write to file: {}", e);
        }
    }
}

async fn get_inbox(actor: &String) -> Result<String, reqwest::Error> {
    let client = reqwest::Client::new();
    let body = client.get(actor).header(reqwest::header::ACCEPT, "application/activity+json")
        .send().await.unwrap().text().await.unwrap();
    let mut object: Value = serde_json::from_str(&body).unwrap();
    Ok(object.get("inbox").unwrap().to_string())
}


async fn post_inbox(actor: &String, body: Value) -> Result<(), reqwest::Error> {
    let client = reqwest::Client::builder()
        .connection_verbose(true)
        .build().unwrap();
    let mut inbox : String = get_inbox(actor).await.unwrap_or(String::new());
    inbox.remove(0); inbox.pop();


    let config = SigningConfig::new("https://cha-cu.it/users/chef#main-key", RsaSha256Sign::new_pem(&*fs::read("privkey.pem").unwrap()).unwrap());
    println!("Send Accept to inbox: {}", inbox);

    let mut req = client
        .post(inbox.clone()).json(&body)
        .header(reqwest::header::ACCEPT, "application/activity+json")
        .build()
        .unwrap()
        .signed(&config)
        .unwrap();

//    println!("@@@ {:?}", req.headers().get(reqwest::header::HeaderName::from_static("Signature")).unwrap());
//    println!("@@@ {:?}", req.headers().get(reqwest::header::HeaderName::from_static("digest")).unwrap());
    let result = client.execute(req).await.unwrap().text().await.unwrap();
    println!("=>{}", result);
    Ok(())
}

// TODO follow
// TODO likes
// TODO check signature
async fn inbox(bytes: Bytes) -> String{
    let body = String::from_utf8(bytes.to_vec()).unwrap();

    let base_obj: ActivityPubRequest = serde_json::from_str(&body).unwrap();

    if base_obj.object_type == "Follow" {
        let follow_obj: FollowObject = serde_json::from_str(&body).unwrap();
        if follow_obj.object != "https://cha-cu.it/users/chef" {
            println!("Unknown object: {}", follow_obj.object);
        } else {
            println!("Get Follow object from {}", follow_obj.actor);
            let mut f = followers();
            f.retain(|x| x != &*follow_obj.actor);
            f.push(follow_obj.actor.clone());
            write_followers(&f);
            // Send accept to inbox of the actor
            post_inbox(&follow_obj.actor, json!({
                "@context":"https://www.w3.org/ns/activitystreams",
                "id":"https://cha-cu.it/users/chef",
                "type":"Accept",
                "actor":"https://cha-cu.it/users/chef",
                "object": follow_obj
            })).await.unwrap();
        }

        return "{}".to_string();
    }

    let request: Value = serde_json::from_str(&body).unwrap();

    if request.get("type").unwrap().as_str().unwrap() == "Undo"
        && request.get("object").unwrap().get("type").unwrap().as_str().unwrap() == "Follow" {
        let mut f = followers();
        let actor = request.get("object").unwrap().get("actor").unwrap().as_str().unwrap();
        println!("Get Unfollow object from {}", actor);
        f.retain(|x| x != &*actor);
        write_followers(&f);
        return String::from("{}");
    } else if request.get("type").unwrap().as_str().unwrap() == "Delete" {
        return String::from("{}");
    }


    println!("{}", body);
    match String::from_utf8(bytes.to_vec()) {
        Ok(text) => format!("{}!\n", text),
        Err(_) => "".to_owned(),
    }
}

/*
async fn inbox(data: web::Json<InboxRequest>) -> &'static str {
    if data.activity_type == "Create" && data.object.object_type == "Article" && data.object.media_type == "text/markdown" {
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
*/

#[derive(Deserialize)]
pub struct OutboxParams {
    page: Option<u32>
}

// TODO test-deploy
// TODO images
// TODO errors
// TODO parametrize cha-cu.it and "chef"

fn update_cache(sorted_recipes: Vec<(&PathBuf, SystemTime)>) {
    let chunked_recipes: Vec<Vec<_>> = sorted_recipes.chunks(12).map(|chunk| chunk.to_vec()).collect();
    let max_page: usize = chunked_recipes.len();
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
                actor: String::new(), // TODO remove
                context: "https://www.w3.org/ns/activitystreams".to_owned(),
                id: format!("https://cha-cu.it/recettes/{}", filename_without_extension).to_owned(),
                object_type: "Article".to_owned(),
                name: match_title.get(1).map_or("ERR", |m| m.as_str()).to_owned(),
                content: markdown,
                attributed_to: "chef@cha-cu.it".to_owned(), // TODO
                media_type: "text/markdown".to_owned(),
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
}

fn get_recipe_paths() -> Vec<PathBuf> {
    fs::read_dir("../content/recettes/")
        .unwrap()
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<Vec<_>, io::Error>>()
        .unwrap()
}

fn sort_recipes_by_modified_time(recipes: &[PathBuf]) -> Vec<(&PathBuf, SystemTime)> {
    let mut sorted = recipes
        .iter()
        .map(|entry| {
            let metadata = entry.metadata().unwrap();
            let modified_time = metadata.modified().unwrap();
            Ok((entry, modified_time))
        })
        .collect::<Result<Vec<_>, io::Error>>()
        .unwrap();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    sorted
}

fn create_cache_directories() -> Result<(), io::Error> {
    fs::create_dir_all(".cache")?;
    fs::create_dir_all("chadiverse")
}

fn outbox_page_0() -> HttpResponse {
    let recipes = get_recipe_paths();
    let max_page: usize = (recipes.len()/12)+1;
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

fn write_date_and_article_count_to_cache(first_entry_date: u64, nb_recipes: usize) -> Result<(), io::Error>  {
    let cache_content = format!("{}\n{}\n", first_entry_date, nb_recipes);
    fs::write(".cache/date_file.txt", cache_content)
}

fn read_date_and_article_count_from_cache() -> Result<(u64, usize), ()> {
    let contents = match fs::read_to_string(".cache/date_file.txt") {
        Ok(contents) => contents,
        Err(_) => return Err(()),
    };

    let lines: Vec<&str> = contents.lines().collect();
    if lines.len() != 2 {
        return Err(());
    }

    let file_date = lines[0].parse::<u64>().unwrap_or(0);
    let file_nb_articles = lines[1].parse().unwrap_or(0 as usize);

    Ok((file_date, file_nb_articles))
}

fn get_first_entry_date(sorted_recipes: &Vec<(&PathBuf, SystemTime)>) -> u64 {
    sorted_recipes[0]
        .1
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn read_cache_for_page(page: usize) -> Result<String, io::Error> {
    fs::read_to_string(format!(".cache/{}.json", page))
}

fn outbox_page(page: usize) -> HttpResponse {
    // Get the list of recipe paths
    let recipes = get_recipe_paths();

    // Sort the recipes by last modification time
    let sorted_recipes = sort_recipes_by_modified_time(&recipes);

    // Create the cache directories if they do not exist
    create_cache_directories().unwrap_or_else(|_| println!("Failed to create directories"));

    let (file_date, file_nb_articles) = read_date_and_article_count_from_cache().unwrap_or((0, 0));

    // Get the date of the first entry
    let first_entry_date = get_first_entry_date(&sorted_recipes);
    if first_entry_date > file_date || file_nb_articles != recipes.len() {
        println!("Refreshing cache.");
        update_cache(sorted_recipes);
        let _ = write_date_and_article_count_to_cache(first_entry_date, recipes.len());
    }

    // Read the cache for the requested page
    let content = read_cache_for_page(page).unwrap_or_else(|_| "{}".to_owned());
    HttpResponse::Ok().json(serde_json::from_str::<serde_json::Value>(&content).unwrap())
}

async fn outbox(info: web::Query<OutboxParams>) -> impl Responder {
    let page: usize = info.page.unwrap_or(0) as usize;
    if page == 0 {
        return outbox_page_0();
    }
    outbox_page(page)
}

// TODO comments
// TODO documentation
async fn profile() -> impl Responder {
    let profile_json = json!({
        "@context": [
          "https://www.w3.org/ns/activitystreams",
          "https://w3id.org/security/v1",
          {
            "manuallyApprovesFollowers": "as:manuallyApprovesFollowers",
            "toot": "http://joinmastodon.org/ns#",
            "featuredTags": {
              "@id": "toot:featuredTags",
              "@type": "@id"
            },
            "alsoKnownAs": {
              "@id": "as:alsoKnownAs",
              "@type": "@id"
            },
            "movedTo": {
              "@id": "as:movedTo",
              "@type": "@id"
            },
            "schema": "http://schema.org#",
            "PropertyValue": "schema:PropertyValue",
            "value": "schema:value",
            "discoverable": "toot:discoverable",
            "Device": "toot:Device",
            "Ed25519Signature": "toot:Ed25519Signature",
            "Ed25519Key": "toot:Ed25519Key",
            "Curve25519Key": "toot:Curve25519Key",
            "EncryptedMessage": "toot:EncryptedMessage",
            "publicKeyBase64": "toot:publicKeyBase64",
            "deviceId": "toot:deviceId",
            "claim": {
              "@type": "@id",
              "@id": "toot:claim"
            },
            "fingerprintKey": {
              "@type": "@id",
              "@id": "toot:fingerprintKey"
            },
            "identityKey": {
              "@type": "@id",
              "@id": "toot:identityKey"
            },
            "devices": {
              "@type": "@id",
              "@id": "toot:devices"
            },
            "messageFranking": "toot:messageFranking",
            "messageType": "toot:messageType",
            "cipherText": "toot:cipherText",
            "suspended": "toot:suspended",
            "focalPoint": {
              "@container": "@list",
              "@id": "toot:focalPoint"
            }
          }
        ],
        "id": "https://cha-cu.it/users/chef",
        "type": "Person",
        "following": "https://cha-cu.it/users/chef/following",
        "followers": "https://cha-cu.it/users/chef/followers",
        "inbox": "https://cha-cu.it/users/chef/inbox",
        "outbox": "https://cha-cu.it/users/chef/outbox",
        "featuredTags": "https://cha-cu.it/tags",
        "preferredUsername": "chef",
        "name": "Kælinn",
        "summary": "Un site de recettes cha-tisfaisantes ! Cha-cuit! est un petit site de partage de recettes en tout genre. Les recettes sont en grande partie réalisées avec l'aide de Kælinn.",
        "url": "https://cha-cu.it/recettes/",
        "manuallyApprovesFollowers": false,
        "discoverable": true,
        "published": "2022-11-11T11:11:11Z",
        "devices": "https://cha-cu.it/users/chef/collections/devices",
        "publicKey": {
          "id": "https://cha-cu.it/users/chef#main-key",
          "owner": "https://cha-cu.it/users/chef",
          "publicKeyPem": "-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAsNjYEeRIPVC3ErOWgQAH\n6zEl/CnkC+mI7MNOPVdlewbp3tPQ0M8aLdO3MRMMXQsxYz6E67MAwUu9M0dVsmvi\ndKHQ7sMyxxysFzAdJp6oM4yA4vQfEZlh8tUTSEn1ZsLzhBP6s+Dr9fxrTs4kfEkZ\nPQbMpFOcjRCkaKKglCfkukL9u1IR97AAqc7WUZd42/x83Ztl2+EIVbNSUL+w0iI5\nDXqZrl0G0yZa9PTLVZXlkKlbQJ2TNVSAAIK75vWw7MU55iBiuzI53FzBA4sMrbuJ\nnLZrFImWyHpFn1M7WzaL45XRG6RXuzMxcjesXyt1nUEfb4pXPdV4Lo2nluJZ8rCX\nLQIDAQAB\n-----END PUBLIC KEY-----" // TODO from letsencrypt
        },
        "tag": [],
        "attachment": [
          {
            "type": "PropertyValue",
            "name": "website",
            "value": "<a href=\"https://cha-cu.it\" target=\"_blank\" rel=\"nofollow noopener noreferrer me\"><span class=\"invisible\">https://</span><span class=\"\">cha-cu.it</span><span class=\"invisible\"></span></a>"
          }
        ],
        "endpoints": {
          "sharedInbox": "https://cha-cu.it/users/chef/inbox"
        },
        "icon": {
            "type": "Image",
            "url": "https://cha-cu.it/chactivityserver/profile.jpg" // TODO parametrize
        },
        "image": {
          "type": "Image",
          "mediaType": "image/jpeg",
          "url": "https://cha-cu.it/logo.png"
        }
      });
    HttpResponse::Ok().json(profile_json)
}


async fn user_followers() -> impl Responder {
    let f = followers();

    let followers_json = json!({
        "@context": [
          "https://www.w3.org/ns/activitystreams",
          "https://w3id.org/security/v1",
          {
            "Emoji": "toot:Emoji",
            "Hashtag": "as:Hashtag",
            "atomUri": "ostatus:atomUri",
            "conversation": "ostatus:conversation",
            "featured": "toot:featured",
            "focalPoint": {
              "@container": "@list",
              "@id": "toot:focalPoint"
            },
            "inReplyToAtomUri": "ostatus:inReplyToAtomUri",
            "manuallyApprovesFollowers": "as:manuallyApprovesFollowers",
            "movedTo": "as:movedTo",
            "ostatus": "http://ostatus.org#",
            "sensitive": "as:sensitive",
            "toot": "http://joinmastodon.org/ns#"
          }
        ],
        "id": "https://cha-cu.it/users/chef/followers",
        "items": f,
        "totalItems": f.len(),
        "type": "OrderedCollection"
    });
    HttpResponse::Ok().json(followers_json)
}

#[derive(Debug, Deserialize)]
pub struct WebFingerRequest {
    resource: String
}

async fn webfinger_handler(info: web::Query<WebFingerRequest>) -> impl Responder {
    if info.resource == "acct:chef@cha-cu.it" {
        return HttpResponse::Ok().json(json!({
            "subject": info.resource,
            "aliases": [
                "https://cha-cu.it/recettes/",
            ],
            "links": [
                {
                    "rel": "http://webfinger.net/rel/profile-page",
                    "type": "text/html",
                    "href": "https://cha-cu.it/recettes"
                },
                {
                    "rel": "self",
                    "type": "application/activity+json",
                    "href": "https://cha-cu.it/users/chef"
                },
                {
                    "rel": "http://ostatus.org/schema/1.0/subscribe",
                    "template": "https://cha-cu.it/api/authorize_interaction?uri={uri}"
                }
            ]
        }));
    }
    HttpResponse::Ok().json(json!({}))
}

fn main() {

    // Init logging
    env_logger::init();

    actix_web::rt::System::with_tokio_rt(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(8)
            .thread_name("main-tokio")
            .build()
            .unwrap()
    })
    .block_on(async_main());
}

async fn async_main() {
    tokio::spawn(async move {
        println!("From main tokio thread");
        // Would panic if uncommented showing no system here
        // println!("{:?}",actix_web::rt::System::current());
    });

    HttpServer::new(|| {
        App::new()
            .route("/users/chef/inbox", web::post().to(inbox))
            .route("/users/chef/outbox", web::get().to(outbox))
            .route("/users/chef", web::get().to(profile))
            .route("/users/chef/followers", web::get().to(user_followers))
            .route("/.well-known/webfinger", web::get().to(webfinger_handler))
    })
    .bind("127.0.0.1:8080").unwrap()
    .run()
    .await
    .unwrap()
}


/*
use actix_web::{HttpResponse, Responder};

async fn handle_request() -> impl Responder {
    let signature = "signature";

    HttpResponse::Ok()
        .with_header("signature", signature)
        .body("Hello, world!")
}

*/
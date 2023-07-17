/**
 *  Copyright (c) 2022-2023, Sébastien Blin <sebastien.blin@enconn.fr>
 *
 * Redistribution and use in source and binary forms, with or without modification,
 * are permitted provided that the following conditions are met:
 *
 * 1. Redistributions of source code must retain the above copyright notice,
 * this list of conditions and the following disclaimer.
 *
 * 2. Redistributions in binary form must reproduce the above copyright notice,
 * this list of conditions and the following disclaimer in the documentation
 * and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
 * AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
 * WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED.
 * IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT,
 * INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
 * BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
 * DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
 * LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE
 * OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF
 * ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 **/
use crate::articleparser::ArticleParser;
use crate::config::Config;
use crate::follow::Followers;
use crate::likes::Likes;
use crate::noteparser::NoteParser;
use crate::profile::Profile;

use actix_web::{
    web::{Bytes, Data, Query},
    HttpRequest, HttpResponse, Responder,
};
use base64::{engine::general_purpose, Engine as _};
use chrono::{offset::Utc, DateTime};
use core::time::Duration;
use http_sig::{HttpSignatureVerify, RsaSha256Verify};
use openssl::sha::Sha256;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::fs::OpenOptions;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use std::{fs, io};

#[derive(Debug, Deserialize)]
pub struct WebFingerRequest {
    pub resource: String,
}

#[derive(Debug, Deserialize)]
pub struct LikesRequest {
    pub object: String,
    pub wanted_type: String,
}

#[derive(Deserialize)]
pub struct OutboxParams {
    pub page: Option<u32>,
}

#[derive(Deserialize, Serialize)]
pub struct ActivityPubRequest {
    #[serde(rename = "type", default)]
    pub object_type: String,
    #[serde(rename = "@context", default)]
    pub context: Value,
}

#[derive(Deserialize, Serialize)]
pub struct LikeObject {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub actor: String,
    #[serde(default)]
    pub object: String,
    #[serde(rename = "type", default)]
    pub object_type: String,
    #[serde(rename = "@context", default)]
    pub context: String,
}

#[derive(Deserialize, Serialize)]
pub struct FollowObject {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub actor: String,
    #[serde(default)]
    pub object: String,
    #[serde(rename = "type", default)]
    pub object_type: String,
    #[serde(rename = "@context", default)]
    pub context: String,
}

#[derive(Debug, Clone)]
pub struct Server {
    pub config: Config,
    pub profile: Profile,
    pub likes: Likes,
    pub note_parser: NoteParser,
    pub article_parser: ArticleParser,
}

pub struct ServerData {
    pub server: Arc<Mutex<Server>>,
    pub config: Config,
    pub followers: Arc<Mutex<Followers>>,
}

impl Server {
    /**
     * Follow https://www.rfc-editor.org/rfc/rfc7033 for WebFinger Discovery
     * /.well-known/webfinger?resource=acct:user@domain.org is called
     * user@domain.org corresponds to our user.
     * The answer (if the user is found) will return the main profile page (recipes)
     * and a activity+json page corresponding to the Fediverse profile.
     * @param server
     * @param web   The incoming web request
     * @return webfinger json response
     */
    pub async fn webfinger(
        data: Data<ServerData>,
        info: Query<WebFingerRequest>,
    ) -> impl Responder {
        log::info!("GET WebFinger request: {}", info.resource);
        let config = data.config.clone();

        if info.resource == format!("acct:{}@{}", config.user, config.domain) {
            return HttpResponse::Ok().json(json!({
                "subject": info.resource,
                "aliases": [
                    format!("https://{}/{}/", config.domain, config.profile),
                ],
                "links": [
                    {
                        "rel": "http://webfinger.net/rel/profile-page",
                        "type": "text/html",
                        "href": format!("https://{}/{}", config.domain, config.profile)
                    },
                    {
                        "rel": "self",
                        "type": "application/activity+json",
                        "href": format!("https://{}/users/{}", config.domain, config.user)
                    }
                ]
            }));
        }
        HttpResponse::Ok().json(json!({}))
    }

    /**
     * Once the webfinger request has been parsed, the next step is to find the
     * inbox/outbox and details of the Object (here an ActivityPub Person).
     * @param server
     * @return the profile page
     */
    pub async fn profile(data: Data<ServerData>) -> impl Responder {
        log::info!("GET Profile");
        let server = data.server.lock().unwrap();
        server.profile.profile()
    }

    /**
     * https://www.w3.org/TR/activitypub/#outbox
     * Contains all articles of the website
     * @todo likes
     * if page isn't provide, a description of the outbox is provided
     * else, a json of 12 articles is sent.
     * To avoid some computation, pages are cached (the cache is invalidated if
     * a new article is detected).
     * @param server
     * @param info      Web request parameters (contains a page number)
     * @return Outbox' json
     */
    pub async fn outbox(data: Data<ServerData>, info: Query<OutboxParams>) -> impl Responder {
        let config: Config;
        let followers: Arc<Mutex<Followers>>;
        {
            config = data.config.clone();
            followers = data.followers.clone();
        }
        let page: usize = info.page.unwrap_or(0) as usize;
        if page == 0 {
            return Server::outbox_page_0(&config);
        }
        Server::check_cache(&config, &followers.lock().unwrap()).await;

        let mut server = data.server.lock().unwrap();
        server.outbox_page(page).await
    }

    /**
     * Returns the list of followers
     * @param server
     * @return json containing a collection of followers
     */
    pub async fn user_followers(data: Data<ServerData>) -> impl Responder {
        log::info!("GET Followers");
        let followers = data.followers.lock().unwrap();
        followers.user_followers()
    }

    /**
     * Returns the list of following
     * @param server
     * @return json containing a collection of following
     */
    pub async fn user_following(data: Data<ServerData>) -> impl Responder {
        log::info!("GET Following users");
        let followers = data.followers.lock().unwrap();
        followers.user_following()
    }

    /**
     * Handle Outbox requests if page == 0
     * @param page
     */
    fn outbox_page_0(config: &Config) -> HttpResponse {
        log::info!("GET Outbox");
        let recipes = Server::get_recipe_paths(&config.input_dir);
        let max_page: usize = (recipes.len() / 12) + 1;
        // If no page provided, then describe the other pages
        let outbox_json = json!({
            "@context": "https://www.w3.org/ns/activitystreams",
            "id": format!("https://{}/users/{}/outbox", config.domain, config.user),
            "type": "OrderedCollection",
            "totalItems": recipes.len(),
            "first": format!("https://{}/users/{}/outbox?page=1", config.domain, config.user),
            "last": format!("https://{}/users/{}/outbox?page={}", config.domain, config.user, max_page),
        });
        return HttpResponse::Ok().json(outbox_json);
    }

    /**
     * Handle Outbox requests if page > 0
     * @param page
     */
    async fn outbox_page(&mut self, page: usize) -> HttpResponse {
        log::info!("GET Outbox page: {}", page);
        // Read the cache for the requested page
        let content = self
            .read_cache_for_page(page)
            .unwrap_or_else(|_| "{}".to_owned());
        HttpResponse::Ok().json(serde_json::from_str::<serde_json::Value>(&content).unwrap())
    }

    /**
     * Because we use a static website, update the cache to announce articles ASAP
     * @param self
     */
    async fn check_cache(config: &Config, followers: &Followers) {
        // TODO check write time for instances/blocked
        // Get the list of recipe paths
        let recipes = Server::get_recipe_paths(&config.input_dir);

        // Sort the recipes by last modification time
        let sorted_recipes = Server::sort_recipes_by_modified_time(&recipes);

        // Create the cache directories if they do not exist
        fs::create_dir_all(&config.cache_dir)
            .unwrap_or_else(|_| log::error!("Failed to create directories"));
        fs::create_dir_all(&config.output_dir)
            .unwrap_or_else(|_| log::error!("Failed to create directories"));

        let (file_date, file_nb_articles) =
            Server::read_date_and_article_count_from_cache(&config.cache_dir).unwrap_or((0, 0));

        // Get the date of the first entry
        let first_entry_date = Server::get_first_entry_date(&sorted_recipes);
        if first_entry_date > file_date || file_nb_articles != recipes.len() {
            log::warn!("Refreshing cache.");
            let to_announce = Server::update_cache(&config, sorted_recipes, file_date).await;
            let _ = Server::write_date_and_article_count_to_cache(
                &config,
                first_entry_date,
                recipes.len(),
            );
            Server::announce_articles(&followers, to_announce).await;
        }
    }

    /**
     * Get public key from a key-id
     */
    async fn get_public_key(key_id: &String) -> Result<String, reqwest::Error> {
        log::info!("Retrieving public key: {}", key_id);
        let client = reqwest::Client::builder()
            .connection_verbose(true)
            .timeout(Duration::new(10, 0))
            .connect_timeout(Duration::new(10, 0))
            .build()?;
        let body = client
            .get(key_id)
            .header(reqwest::header::ACCEPT, "application/activity+json")
            .send()
            .await;
        if body.is_ok() {
            return Ok(String::new());
        }
        let body = body.unwrap().text().await;
        if !body.is_ok() {
            return Ok(String::new());
        }
        let obj = serde_json::from_str(&body.unwrap());
        if !obj.is_ok() {
            log::warn!("Incorrect object for: {}", key_id);
            return Ok(String::new());
        }
        let object: Value = obj.unwrap();
        let pk = object.get("publicKey");
        if pk.is_none() {
            log::warn!("Incorrect object for: {}", key_id);
            return Ok(String::new());
        }
        let pk = pk.unwrap().get("publicKeyPem");
        if pk.is_none() {
            log::warn!("Incorrect object for: {}", key_id);
            return Ok(String::new());
        }
        Ok(pk.unwrap().as_str().unwrap().to_owned())
    }

    async fn verify(req: HttpRequest, body: &Bytes) -> bool {
        // First, check that the request is less than twelve hours old
        let date = req.headers().get("date");
        if date.is_none() {
            log::error!("Verification Failed: header date is missing");
            return false;
        }
        let date = date.unwrap().to_str().ok().unwrap();
        let date: DateTime<Utc> = DateTime::parse_from_rfc2822(date).unwrap().into();
        let now = Utc::now();
        let diff = now - date;
        if diff.num_hours() > 12 {
            log::error!("Verification Failed: too old request");
            return false;
        }

        // Verify http signature
        let signature = req.headers().get("signature");
        if signature.is_some() {
            let sign_header = signature.unwrap().to_str().ok().unwrap();
            // Parse the auth params
            let auth_args = sign_header
                .split(',')
                .map(|part: &str| {
                    let mut kv = part.splitn(2, '=');
                    let k = kv.next()?.trim();
                    let v = kv.next()?.trim().trim_matches('"');
                    Some((k, v))
                })
                .collect::<Option<BTreeMap<_, _>>>()
                .or_else(|| {
                    log::error!("Verification Failed: Unable to parse 'Signature' header");
                    None
                })
                .unwrap();
            let key_id = *auth_args
                .get("keyId")
                .or_else(|| {
                    log::error!(
                        "Verification Failed: Missing required 'keyId' in 'Authorization' header"
                    );
                    None
                })
                .unwrap();
            let provided_signature = auth_args
                .get("signature")
                .or_else(|| {
                    log::error!("Verification Failed: Missing required 'signature' in 'Authorization' header");
                    None
                })
                .unwrap();
            let algorithm_name = auth_args.get("algorithm").copied().unwrap();
            if algorithm_name != "rsa-sha256" && algorithm_name != "hs2019" {
                log::error!("Verification Failed: Invalid algorithm {}", algorithm_name);
                return false;
            }
            let digest_header = req.headers().get("digest").unwrap().to_str().ok().unwrap();
            let digest_header = &digest_header[(digest_header.find('=').unwrap_or(0) + 1)..];
            let headers: Vec<String> = auth_args
                .get("headers")
                .unwrap()
                .split(' ')
                .map(|h| h.to_owned())
                .collect();
            let mut to_sign = Vec::new();
            for header in headers.iter() {
                if header == "(request-target)" {
                    // TODO (creates)/(verification)
                    to_sign.push(format!("(request-target): post {}", req.path()));
                } else {
                    to_sign.push(format!(
                        "{}: {}",
                        header,
                        req.headers().get(header).unwrap().to_str().ok().unwrap()
                    ));
                }
            }

            // Retrieve key
            let pk = Server::get_public_key(&key_id.to_owned())
                .await
                .unwrap_or(String::new());
            if pk == "" {
                log::error!("Verification Failed: no public key for: {}", key_id);
                return false;
            }
            // TODO cache with expiration
            // TODO crash proof

            // Verify digest
            let mut sha256 = Sha256::new();
            sha256.update(body);
            let hash = sha256.finish();
            let digest: String = general_purpose::STANDARD_NO_PAD.encode(hash);
            if digest == digest_header {
                log::error!("Verification Failed: Invalid Digest from {}", key_id);
                return false;
            }

            let verificator = RsaSha256Verify::new_pem(pk.as_bytes()).unwrap();
            let res = verificator.http_verify(to_sign.join("\n").as_bytes(), &*provided_signature);
            if !res {
                log::error!("Verification Failed: Invalid Signature from {}", key_id);
            }
            return res;
        }
        log::error!("Verification Failed: no signature found");
        false
    }

    /**
     * User's inbox. This will receive all object from the fediverse (articles/messages/follow requests/likes)
     * For now, only follow requests are supported
     */
    pub async fn inbox(data: Data<ServerData>, bytes: Bytes, req: HttpRequest) -> String {
        if !Server::verify(req, &bytes).await {
            return String::from("");
        }
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        let base_obj: ActivityPubRequest = serde_json::from_str(&body).unwrap();
        let server: Arc<Mutex<Server>>;
        {
            server = data.server.clone();
            let fwrs_arc = data.followers.clone();
            let mut fwrs = fwrs_arc.lock().unwrap();

            // Update cache for instances
            let current_blocked = fwrs.blocked.clone();
            let mut followed_instances = Vec::new();
            fwrs.update_cache(&mut followed_instances).await;
            let current_blocked: HashSet<_> = current_blocked.iter().collect();
            let new_blocked: Vec<_> = fwrs
                .blocked
                .clone()
                .into_iter()
                .filter(|item| !current_blocked.contains(item))
                .collect();
            server.lock().unwrap().note_parser.clear_user(&new_blocked);
            server.lock().unwrap().article_parser.clear_user(&new_blocked);

            for actor in followed_instances {
                // For new instances, get last articles
                let best_name = Followers::get_best_name(&actor)
                    .await
                    .unwrap_or(String::new());
                let _ = server.lock().unwrap()
                    .parse_outbox(&Followers::get_outbox(&actor).await.unwrap(), &best_name)
                    .await;
            }

        }
        let config = data.config.clone();
        let followers = data.followers.clone();

        Server::check_cache(&config, &followers.lock().unwrap()).await;

        if base_obj.object_type == "Follow" {
            let follow_obj: FollowObject = serde_json::from_str(&body).unwrap();
            if follow_obj.object != format!("https://{}/users/{}", config.domain, config.user) {
                println!("Unknown object: {}", follow_obj.object);
            } else if followers.lock().unwrap().is_blocked(&follow_obj.actor) {
                return String::from("{}");
            } else {
                println!("Get Follow object from {} {}", follow_obj.actor, body);
                let inbox = Followers::get_inbox(&follow_obj.actor, false)
                    .await
                    .unwrap_or(String::new());
                followers.lock().unwrap().accept(&follow_obj, &inbox).await;
                if config.auto_follow_back {
                    followers
                        .lock()
                        .unwrap()
                        .send_follow(&follow_obj.actor, &inbox)
                        .await;
                }
            }
            return String::from("{}");
        } else if base_obj.object_type == "Accept" {
            let object: Value = serde_json::from_str(&body).unwrap();
            if object
                .get("object")
                .unwrap()
                .get("type")
                .unwrap()
                .as_str()
                .unwrap()
                == "Follow"
            {
                let follow_obj: FollowObject =
                    serde_json::from_value(object.get("object").unwrap().clone()).unwrap();
                followers
                    .lock()
                    .unwrap()
                    .follow_accepted(&follow_obj.object)
                    .await;
            }
            return String::from("{}");
        } else if base_obj.object_type == "Like" {
            let like_obj: LikeObject = serde_json::from_str(&body).unwrap();
            if like_obj.object.contains(&config.domain) {
                println!("Like {} from {}", like_obj.object, like_obj.actor);
                let mut server = server.lock().unwrap();
                server.likes.like(&like_obj.object, &like_obj.actor);
            }
            return String::from("{}");
        } else if base_obj.object_type == "Announce" {
            let announce_obj: LikeObject = serde_json::from_str(&body).unwrap();
            if announce_obj.object.contains(&config.domain) {
                println!("Boost {} from {}", announce_obj.object, announce_obj.actor);
                let mut server = server.lock().unwrap();
                server
                    .likes
                    .boost(&announce_obj.object, &announce_obj.actor);
            }
            return String::from("{}");
        } else if base_obj.object_type == "Create" {
            let base_obj: Value = serde_json::from_str(&body).unwrap();
            let actor = base_obj
                .get("actor")
                .unwrap()
                .as_str()
                .unwrap_or("")
                .to_owned();
            if followers.lock().unwrap().is_blocked(&actor) {
                return String::from("{}");
            }
            let base_obj: Value = base_obj.get("object").unwrap().to_owned();
            let obj_type = base_obj.get("type").unwrap().as_str().unwrap_or("");
            if obj_type == "Note" {
                // Check that we follow author
                if followers.lock().unwrap().is_following(&actor) {
                    let best_name = Followers::get_best_name(&actor)
                        .await
                        .unwrap_or(String::new());
                    let mut server = server.lock().unwrap();
                    if server.note_parser.parse(base_obj.clone(), best_name) {
                        return String::from("{}");
                    }
                }
                if base_obj.get("cc").is_none() || base_obj.get("inReplyToAtomUri").is_none() {
                    return String::from("{}");
                }
                let cc = base_obj.get("cc").unwrap().as_array().unwrap();
                if cc.contains(&json!(format!(
                    "https://{}/users/{}",
                    config.domain, config.user
                ))) {
                    let reply_to = base_obj
                        .get("inReplyToAtomUri")
                        .unwrap()
                        .as_str()
                        .unwrap_or("");
                    let html_content = base_obj.get("content").unwrap().as_str().unwrap_or("");
                    let content =
                        html2text::from_read(&html_content.as_bytes()[..], html_content.len());
                    let reply = format!("{} - {}: {}", reply_to, actor, content);
                    println!("{}", reply);
                    let path = format!("{}/mentions", config.cache_dir);
                    if !Path::new(&path).exists() {
                        let _file = OpenOptions::new()
                            .create_new(true)
                            .open(path.clone())
                            .unwrap();
                    }

                    let mut file = OpenOptions::new()
                        .write(true)
                        .append(true)
                        .open(path)
                        .unwrap();
                    if let Err(e) = writeln!(file, "{}", reply) {
                        eprintln!("Couldn't write to file: {}", e);
                    }
                }
                return String::from("{}");
            } else if obj_type == "Article" {
                // Check that we follow author
                if followers.lock().unwrap().is_following(&actor) {
                    let mut server = server.lock().unwrap();
                    server.article_parser.parse(
                        base_obj,
                        Followers::get_best_name(&actor)
                            .await
                            .unwrap_or(String::new()),
                    );
                }
            }
            println!("{}", body);
            return String::from("{}");
        }

        let request: Value = serde_json::from_str(&body).unwrap();
        if request.get("type").unwrap().as_str().unwrap() == "Undo" {
            let base_object = request.get("object").unwrap();
            let obj_type = base_object.get("type").unwrap().as_str().unwrap();
            if obj_type == "Follow" {
                let actor = base_object.get("actor").unwrap().as_str().unwrap();
                println!("Get Unfollow object from {}", actor);
                followers.lock().unwrap().unfollow(&actor.to_owned());
            } else if obj_type == "Like" {
                let object = base_object
                    .get("object")
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string();
                let actor = base_object
                    .get("actor")
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string();

                if object.contains(&config.domain) {
                    println!("UnLike {} from {}", object, actor);
                    let mut server = server.lock().unwrap();
                    server.likes.unlike(&object, &actor);
                }
            } else if obj_type == "Announce" {
                let object = base_object
                    .get("object")
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string();
                let actor = base_object
                    .get("actor")
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string();
                if object.contains(&config.domain) {
                    println!("UnBoost {} from {}", object, actor);
                    let mut server = server.lock().unwrap();
                    server.likes.unboost(&object, &actor);
                }
            }
            return String::from("{}");
        } else if request.get("type").unwrap().as_str().unwrap() == "Delete" {
            return String::from("{}");
        }

        println!("{}", body);
        match String::from_utf8(bytes.to_vec()) {
            Ok(text) => format!("{}\n", text),
            Err(_) => "".to_owned(),
        }
    }

    /**
     * Returns likes and boost per recipe
     * @param server
     * @param info         object is the url of the recipe and wanted_type (like/boost)
     * @return the array of people who boost/like the recipe
     */
    pub async fn likes(server: Data<Mutex<Server>>, info: Query<LikesRequest>) -> impl Responder {
        let server = server.lock().unwrap();
        HttpResponse::Ok().json(server.likes.data(&info.object, &info.wanted_type))
    }

    // Utils
    fn get_recipe_paths(path: &String) -> Vec<PathBuf> {
        fs::read_dir(path)
            .unwrap()
            .map(|res| res.map(|e| e.path()))
            .collect::<Result<Vec<_>, io::Error>>()
            .unwrap()
    }

    fn get_images(image_dir: &String, recipe: String) -> Vec<PathBuf> {
        let path = format!("{}/{}", image_dir, recipe);
        if Path::new(&path).exists() {
            return fs::read_dir(&path)
                .unwrap()
                .map(|res| res.map(|e| e.path()))
                .collect::<Result<Vec<_>, io::Error>>()
                .unwrap();
        }
        Vec::new()
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

    fn write_date_and_article_count_to_cache(
        config: &Config,
        first_entry_date: u64,
        nb_recipes: usize,
    ) -> Result<(), io::Error> {
        let cache_content = format!("{}\n{}\n", first_entry_date, nb_recipes);
        fs::write(format!("{}/date_file.txt", config.cache_dir), cache_content)
    }

    fn read_date_and_article_count_from_cache(cache_dir: &String) -> Result<(u64, usize), ()> {
        let contents = match fs::read_to_string(format!("{}/date_file.txt", cache_dir)) {
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

    fn read_cache_for_page(&self, page: usize) -> Result<String, io::Error> {
        fs::read_to_string(format!("{}/{}.json", self.config.cache_dir, page))
    }

    async fn parse_outbox(
        &mut self,
        outbox: &String,
        best_name: &String,
    ) -> Result<(), reqwest::Error> {
        let client = reqwest::Client::new();
        let mut body = client
            .get(outbox)
            .header(reqwest::header::ACCEPT, "application/activity+json")
            .send()
            .await?
            .text()
            .await?;
        let mut object: Value = serde_json::from_str(&body).unwrap();

        let mut nb_articles = 0;
        let mut pages = Vec::new();

        loop {
            let next_page;
            if object.get("first").is_some() {
                next_page = object.get("first").unwrap().as_str().unwrap().to_owned();
            } else if object.get("next").is_some() {
                next_page = object.get("next").unwrap().as_str().unwrap().to_owned();
            } else {
                break;
            }
            if pages.contains(&next_page) {
                println!("Loop detected: {}", next_page);
                break;
            }
            if object.get("items").is_some() {
                for article in object.get("items").unwrap().as_array().unwrap() {
                    if article.get("object").is_some() {
                        self.article_parser.parse(
                            article.get("object").unwrap().to_owned(),
                            best_name.to_owned(),
                        );
                        nb_articles += 1;
                        if nb_articles > 1000 {
                            return Ok(()); // Avoid too many articles
                        }
                    }
                }
            }
            pages.push(next_page.clone());
            body = client
                .get(next_page)
                .header(reqwest::header::ACCEPT, "application/activity+json")
                .send()
                .await?
                .text()
                .await?;
            object = serde_json::from_str(&body).unwrap();
        }
        Ok(())
    }

    async fn update_cache(
        config: &Config,
        sorted_recipes: Vec<(&PathBuf, SystemTime)>,
        previous_entry_date: u64,
    ) -> Vec<Value> {
        let chunked_recipes: Vec<Vec<_>> = sorted_recipes
            .chunks(12)
            .map(|chunk| chunk.to_vec())
            .collect();
        let max_page: usize = chunked_recipes.len();
        let mut idx_page = 1;
        const HEADER_TITLE_REGEX: &str = r#"title: ([^\n]+)\n"#;
        const HEADER_TAGS_REGEX: &str = r#"tags: \[([^\n]+)\]\n"#;
        let mut to_announce: Vec<Value> = Vec::new();
        let re_title_regex = Regex::new(HEADER_TITLE_REGEX).unwrap();
        let re_tags_regex = Regex::new(HEADER_TAGS_REGEX).unwrap();
        for chunk in chunked_recipes {
            let mut articles = Vec::new();
            // Parse the markdown files into a collection of `Object`s.
            for markdown_file in chunk.iter() {
                let filename_without_extension =
                    markdown_file.0.file_stem().unwrap().to_str().unwrap();
                let markdown = fs::read_to_string(markdown_file.0).unwrap();
                let match_title = re_title_regex.captures(&markdown).unwrap();
                let datetime: DateTime<Utc> = markdown_file.1.into();
                let published = datetime.format("%+").to_string();
                let entry_date = markdown_file
                    .1
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let mut attachments: Vec<Value> = Vec::new();
                for image in
                    Server::get_images(&config.image_dir, filename_without_extension.to_string())
                {
                    attachments.push(json!({
                        "type": "Image",
                        "mediaType": "image/jpeg", // TODO png
                        "url": format!("https://{}/{}{}/{}", config.domain, config.static_image_dir, filename_without_extension, image.file_name().unwrap().to_str().unwrap()),
                    }));
                }
                let mut tags_value: Vec<Value> = Vec::new();
                let mut tags = config.tags.clone();
                if tags.len() == 0 {
                    let tags_article = re_tags_regex.captures(&markdown).unwrap();
                    let tags_article = tags_article.get(1).map_or("", |m| m.as_str()).to_owned();
                    let tags_article: Vec<&str> = tags_article.split(',').collect();
                    tags = tags_article.iter().map(|&s| s.into()).collect();
                }
                for tag in tags {
                    let mut tag = String::from(tag);
                    tag = tag.replace("\"", "");
                    tag = tag.replace(" ", "");
                    tags_value.push(json!({
                        "type": "Hashtag",
                        "href": format!("https://{}/tags/{}", config.domain, tag),
                        "name": format!("#{}", tag)
                    }));
                }
                let article = json!({
                    "@context": [
                        "https://www.w3.org/ns/activitystreams",
                        {
                            "ostatus": "http://ostatus.org#",
                            "atomUri": "ostatus:atomUri",
                            "inReplyToAtomUri": "ostatus:inReplyToAtomUri",
                            "conversation": "ostatus:conversation",
                            "sensitive": "as:sensitive",
                            "toot": "http://joinmastodon.org/ns#",
                            "votersCount": "toot:votersCount"
                        }
                    ],
                    "id": format!("https://{}/recettes/{}", config.domain, filename_without_extension),
                    "type": "Create",
                    "actor": format!("https://{}/users/{}", config.domain, config.user),
                    "published": published,
                    "to": [
                        "https://www.w3.org/ns/activitystreams#Public"
                    ],
                    "cc": [
                        format!("https://{}/users/{}/followers", config.domain, config.user),
                    ],
                    "object": {
                        "id": format!("https://{}/recettes/{}", config.domain, filename_without_extension),
                        "type": "Article",
                        "summary": null,
                        "inReplyTo": null,
                        "published": published,
                        "url": format!("https://{}/recettes/{}", config.domain, filename_without_extension),
                        "attributedTo": format!("https://{}/users/{}", config.domain, config.user),
                        "to": [
                            "https://www.w3.org/ns/activitystreams#Public"
                        ],
                        "cc": [
                            format!("https://{}/users/{}/followers", config.domain, config.user),
                        ],
                        "sensitive": false,
                        "atomUri": format!("https://{}/recettes/{}", config.domain, filename_without_extension),
                        "content": markdown,
                        "name": match_title.get(1).map_or("Chalut!", |m| m.as_str()).to_owned(),
                        "mediaType": String::from("text/markdown"),
                        "attachment": attachments,
                        "tag": tags_value,
                        "license": config.license
                    }
                });
                if entry_date > previous_entry_date {
                    to_announce.push(article.clone());
                }
                articles.push(article);
            }

            let mut outbox_json = json!({
                "type": "OrderedCollectionPage",
                "partOf": format!("https://{}/users/{}/outbox", config.domain, config.user),
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
                "items": articles
            });
            if idx_page > 0 {
                outbox_json["prev"] = serde_json::Value::String(format!(
                    "https://{}/users/{}/outbox?page={}",
                    config.domain,
                    config.user,
                    idx_page - 1
                ));
            }
            if idx_page < max_page {
                outbox_json["next"] = serde_json::Value::String(format!(
                    "https://{}/users/{}/outbox?page={}",
                    config.domain,
                    config.user,
                    idx_page + 1
                ));
            }

            // Cache file
            std::fs::write(
                format!("{}/{}.json", &config.cache_dir, idx_page + 1),
                serde_json::to_string_pretty(&outbox_json).unwrap(),
            )
            .unwrap();
            idx_page += 1;
        }
        to_announce
    }

    /**
     * Announce new articles to followers
     * @param self
     * @param to_announce   Articles to announce
     */
    async fn announce_articles(followers: &Followers, to_annnounce: Vec<Value>) {
        let mut inboxes = HashSet::new();
        for follower in &followers.followers {
            let inbox = Followers::get_inbox(&follower, true)
                .await
                .unwrap_or(String::new());
            if inbox.len() != 0 {
                inboxes.insert(inbox);
            }
        }
        // Get inbox from followers
        for article in &to_annnounce {
            // For each article, post to inboxes
            for inbox in &inboxes {
                println!("Announce {} to {}", article["id"].as_str().unwrap(), inbox);
                let _ = followers.post_inbox(&inbox, article.clone()).await;
            }
        }
    }
}

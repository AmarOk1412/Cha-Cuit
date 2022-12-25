/**
 * Copyright (c) 2022, Sébastien Blin <sebastien.blin@enconn.fr>
 * All rights reserved.
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions are met:
 *
 * * Redistributions of source code must retain the above copyright
 *  notice, this list of conditions and the following disclaimer.
 * * Redistributions in binary form must reproduce the above copyright
 *  notice, this list of conditions and the following disclaimer in the
 *  documentation and/or other materials provided with the distribution.
 * * Neither the name of the University of California, Berkeley nor the
 *  names of its contributors may be used to endorse or promote products
 *  derived from this software without specific prior written permission.
 *
 * THIS SOFTWARE IS PROVIDED BY THE REGENTS AND CONTRIBUTORS ``AS IS'' AND ANY
 * EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
 * WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
 * DISCLAIMED. IN NO EVENT SHALL THE REGENTS AND CONTRIBUTORS BE LIABLE FOR ANY
 * DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
 * (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;
 * LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND
 * ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
 * (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
 * SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 **/
use crate::config::Config;
use crate::follow::Followers;
use crate::profile::Profile;

use actix_web::{HttpResponse, Responder, web::{Bytes, Data, Query}};
use chrono::offset::Utc;
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Value;
use regex::Regex;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;

#[derive(Debug, Deserialize)]
pub struct WebFingerRequest {
    pub resource: String
}

#[derive(Deserialize)]
pub struct OutboxParams {
    pub page: Option<u32>
}

#[derive(Deserialize, Serialize)]
pub struct ActivityPubRequest {
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
    pub followers: Followers,
    pub profile: Profile,
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
    pub async fn webfinger(server: Data<Mutex<Server>>, info: Query<WebFingerRequest>) -> impl Responder {
        let server = server.lock().unwrap();
        if info.resource == format!("acct:{}@{}", server.config.user, server.config.domain) {
            return HttpResponse::Ok().json(json!({
                "subject": info.resource,
                "aliases": [
                    format!("https://{}/{}/", server.config.domain, server.config.profile),
                ],
                "links": [
                    {
                        "rel": "http://webfinger.net/rel/profile-page",
                        "type": "text/html",
                        "href": format!("https://{}/{}", server.config.domain, server.config.profile)
                    },
                    {
                        "rel": "self",
                        "type": "application/activity+json",
                        "href": format!("https://{}/users/{}", server.config.domain, server.config.user)
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
    pub async fn profile(server: Data<Mutex<Server>>) -> impl Responder {
        let server = server.lock().unwrap();
        server.profile.profile()
    }

    /**
     * https://www.w3.org/TR/activitypub/#outbox
     * Contains all articles of the website
     * @todo likes
     * @todo images from articles
     * if page isn't provide, a description of the outbox is provided
     * else, a json of 12 articles is sent.
     * To avoid some computation, pages are cached (the cache is invalidated if
     * a new article is detected).
     * @param server
     * @param info      Web request parameters (contains a page number)
     * @return Outbox' json
     */
    pub async fn outbox(server: Data<Mutex<Server>>, info: Query<OutboxParams>) -> impl Responder {
        let server = server.lock().unwrap();
        let page: usize = info.page.unwrap_or(0) as usize;
        if page == 0 {
            return server.outbox_page_0();
        }
        server.outbox_page(page)
    }

    /**
     * Returns the list of followers
     * @param server
     * @return json containing a collection of followers
     */
    pub async fn user_followers(server: Data<Mutex<Server>>) -> impl Responder {
        let server = server.lock().unwrap();
        server.followers.user_followers()
    }

    /**
     * Handle Outbox requests if page == 0
     * @param page
     */
    fn outbox_page_0(&self) -> HttpResponse {
        let recipes = self.get_recipe_paths();
        let max_page: usize = (recipes.len()/12)+1;
        // If no page provided, then describe the other pages
        let outbox_json = json!({
            "@context": "https://www.w3.org/ns/activitystreams",
            "id": format!("https://{}/users/{}/outbox", self.config.domain, self.config.user),
            "type": "OrderedCollection",
            "totalItems": recipes.len(),
            "first": format!("https://{}/users/{}/outbox?page=1", self.config.domain, self.config.user),
            "last": format!("https://{}/users/{}/outbox?page={}", self.config.domain, self.config.user, max_page),
        });
        return HttpResponse::Ok().json(outbox_json);
    }

    /**
     * Handle Outbox requests if page > 0
     * @param page
     */
    fn outbox_page(&self, page: usize) -> HttpResponse {
        // Get the list of recipe paths
        let recipes = self.get_recipe_paths();

        // Sort the recipes by last modification time
        let sorted_recipes = Server::sort_recipes_by_modified_time(&recipes);

        // Create the cache directories if they do not exist
        self.create_cache_directories().unwrap_or_else(|_| println!("Failed to create directories"));

        let (file_date, file_nb_articles) = self.read_date_and_article_count_from_cache().unwrap_or((0, 0));

        // Get the date of the first entry
        let first_entry_date = Server::get_first_entry_date(&sorted_recipes);
        if first_entry_date > file_date || file_nb_articles != recipes.len() {
            println!("Refreshing cache.");
            self.update_cache(sorted_recipes);
            let _ = self.write_date_and_article_count_to_cache(first_entry_date, recipes.len());
        }

        // Read the cache for the requested page
        let content = self.read_cache_for_page(page).unwrap_or_else(|_| "{}".to_owned());
        HttpResponse::Ok().json(serde_json::from_str::<serde_json::Value>(&content).unwrap())
    }

    /**
     * User's inbox. This will receive all object from the fediverse (articles/messages/follow requests/likes)
     * For now, only follow requests are supported
     * @todo receive articles
     * @todo receive likes
     */
    pub async fn inbox(server: Data<Mutex<Server>>, bytes: Bytes) -> String {
        let server = server.lock().unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        let base_obj: ActivityPubRequest = serde_json::from_str(&body).unwrap();

        if base_obj.object_type == "Follow" {
            let follow_obj: FollowObject = serde_json::from_str(&body).unwrap();
            if follow_obj.object != "https://cha-cu.it/users/chef" {
                println!("Unknown object: {}", follow_obj.object);
            } else {
                println!("Get Follow object from {}", follow_obj.actor);
                let mut inbox : String = Server::get_inbox(&follow_obj.actor).await.unwrap_or(String::new());
                inbox.remove(0); inbox.pop();
                server.followers.accept(follow_obj, inbox).await;
            }
            return String::from("{}");
        }

        let request: Value = serde_json::from_str(&body).unwrap();
        if request.get("type").unwrap().as_str().unwrap() == "Undo"
            && request.get("object").unwrap().get("type").unwrap().as_str().unwrap() == "Follow" {
            let mut f = server.followers.followers();
            let actor = request.get("object").unwrap().get("actor").unwrap().as_str().unwrap();
            println!("Get Unfollow object from {}", actor);
            f.retain(|x| x != &*actor);
            server.followers.write_followers(&f);
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

    // Utils
    fn get_recipe_paths(&self) -> Vec<PathBuf> {
        fs::read_dir(&self.config.input_dir)
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

    fn create_cache_directories(&self) -> Result<(), io::Error> {
        fs::create_dir_all(&self.config.cache_dir)?;
        fs::create_dir_all(&self.config.output_dir)
    }

    fn write_date_and_article_count_to_cache(&self, first_entry_date: u64, nb_recipes: usize) -> Result<(), io::Error>  {
        let cache_content = format!("{}\n{}\n", first_entry_date, nb_recipes);
        fs::write(format!("{}/date_file.txt", self.config.cache_dir), cache_content)
    }

    fn read_date_and_article_count_from_cache(&self) -> Result<(u64, usize), ()> {
        let contents = match fs::read_to_string(format!("{}/date_file.txt", self.config.cache_dir)) {
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

    fn update_cache(&self, sorted_recipes: Vec<(&PathBuf, SystemTime)>) {
        let chunked_recipes: Vec<Vec<_>> = sorted_recipes.chunks(12).map(|chunk| chunk.to_vec()).collect();
        let max_page: usize = chunked_recipes.len();
        let mut idx_page = 0;
        const HEADER_TITLE_REGEX: &str = r#"title: ([^\n]+)\n"#;
        let re_title_regex = Regex::new(HEADER_TITLE_REGEX).unwrap();
        for chunk in chunked_recipes {
            let mut articles = Vec::new();
            // Parse the markdown files into a collection of `Object`s.
            for markdown_file in chunk.iter() {
                let filename_without_extension = markdown_file.0.file_stem().unwrap().to_str().unwrap();
                let markdown = fs::read_to_string(markdown_file.0).unwrap();
                let match_title = re_title_regex.captures(&markdown).unwrap();
                let datetime: DateTime<Utc> = markdown_file.1.into();
                let published = datetime.format("%+").to_string();
                let article = json!({
                    "actor": format!("https://{}/users/{}", self.config.domain, self.config.user),
                    "cc": [], // TODO from followers
                    "id": format!("https://{}/recettes/{}", self.config.domain, filename_without_extension),
                    "to": [
                        "https://www.w3.org/ns/activitystreams#Public"
                    ],
                    "type": "Create",
                    "object": {
                        "to": [
                            "https://www.w3.org/ns/activitystreams#Public"
                        ],
                        "type": String::from("Article"),
                        "url": format!("https://{}/recettes/{}", self.config.domain, filename_without_extension),
                        "name": match_title.get(1).map_or("ERR", |m| m.as_str()).to_owned(),
                        "id": format!("https://{}/recettes/{}", self.config.domain, filename_without_extension),
                        "attributedTo": format!("{}@{}", self.config.user, self.config.domain),
                        "mediaType": "text/markdown".to_owned(),
                        "content": markdown,
                        "license": self.config.license,
                        "published": published,
                    }
                });
                articles.push(article);
            }

            let mut outbox_json = json!({
                "type": "OrderedCollectionPage",
                "partOf": format!("https://{}/users/{}/outbox", self.config.domain, self.config.user),
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
            if idx_page > 1 {
                outbox_json["prev"] = serde_json::Value::String(format!("https://{}/users/{}/outbox?page={}", self.config.domain, self.config.user, idx_page - 1));
            }
            if idx_page < max_page {
                outbox_json["next"] = serde_json::Value::String(format!("https://{}/users/{}/outbox?page={}", self.config.domain, self.config.user, idx_page + 1));
            }

            // Cache file
            std::fs::write(
                format!(".cache/{}.json", idx_page + 1),
                serde_json::to_string_pretty(&outbox_json).unwrap(),
            ).unwrap();
            idx_page += 1;
        }
    }

    /**
     * Get inbox from fediverse actor
     */
    async fn get_inbox(actor: &String) -> Result<String, reqwest::Error> {
        let client = reqwest::Client::new();
        let body = client.get(actor).header(reqwest::header::ACCEPT, "application/activity+json")
            .send().await.unwrap().text().await.unwrap();
        let object: Value = serde_json::from_str(&body).unwrap();
        Ok(object.get("inbox").unwrap().to_string())
    }
}
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
use crate::config::Config;
use crate::server::FollowObject;

use actix_web::{HttpResponse, Responder};
use http_sig::*;
use rand::{distributions::Alphanumeric, Rng};
use serde_json::json;
use serde_json::Value;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::prelude::*;
use std::io::BufReader;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Followers {
    pub config: Config,
    pub following: Vec<String>,
    pub pending_following: Vec<String>,
}

impl Followers {
    /**
     * This structure is used to store followers and following objects
     * Data is serialized in .cache/following.json, .cache/pending_following.json
     */
    pub fn new(config: Config) -> Followers {
        let mut following = Vec::new();
        let mut pending_following = Vec::new();
        let path = format!("{}/following.json", config.cache_dir);
        if Path::new(&path).exists() {
            following =
                serde_json::from_str(&*fs::read_to_string(path).unwrap_or(String::new())).unwrap();
        }
        let path = format!("{}/pending_following.json", config.cache_dir);
        if Path::new(&path).exists() {
            pending_following =
                serde_json::from_str(&*fs::read_to_string(path).unwrap_or(String::new())).unwrap();
        }

        Followers {
            config,
            following,
            pending_following,
        }
    }

    /**
     * This object stores a list of users followed
     * @param self
     */
    pub fn user_following(&self) -> impl Responder {
        let following_json = json!({
            "@context": [
              "https://www.w3.org/ns/activitystreams",
              "https://w3id.org/security/v1",
              Followers::mastodon_value()
            ],
            "id": format!("https://{}/users/{}/following", self.config.domain, self.config.user),
            "items": self.following,
            "totalItems": self.following.len(),
            "type": "OrderedCollection"
        });
        HttpResponse::Ok().json(following_json)
    }

    /**
     * This object stores a list of followers of the instance
     * @param self
     */
    pub fn user_followers(&self) -> impl Responder {
        let f = self.followers();

        let followers_json = json!({
            "@context": [
              "https://www.w3.org/ns/activitystreams",
              "https://w3id.org/security/v1",
              Followers::mastodon_value()
            ],
            "id": format!("https://{}/users/{}/followers", self.config.domain, self.config.user),
            "items": f,
            "totalItems": f.len(),
            "type": "OrderedCollection"
        });
        HttpResponse::Ok().json(followers_json)
    }

    /**
     * @todo only serialize once and update the file
     * instead of rewritting it
     */
    pub fn followers(&self) -> Vec<String> {
        let mut followers: Vec<String> = Vec::new();
        if Path::new(&*format!("{}/followers", self.config.cache_dir)).exists() {
            let file = File::open(format!("{}/followers", self.config.cache_dir)).unwrap();
            let buf = BufReader::new(file);
            followers = buf
                .lines()
                .map(|l| l.expect("Could not parse line"))
                .collect();
        }
        followers
    }

    /**
     * @todo only serialize once and update the file
     * instead of rewritting it
     */
    pub fn write_followers(&self, followers: &Vec<String>) {
        // TODO serialize and refresh instead removing
        let _ = fs::remove_file(&*format!("{}/followers", self.config.cache_dir));
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .append(false)
            .open(format!("{}/followers", self.config.cache_dir))
            .unwrap();
        for follower in followers {
            if let Err(e) = writeln!(file, "{}", follower) {
                eprintln!("Couldn't write to file: {}", e);
            }
        }
    }

    /**
     * Accepting a follow request will update the followers list
     * and also send an Accept object to the actor's inbox
     * @param self
     * @param follow_object
     * @param actor_inbox
     */
    pub async fn accept(&self, follow_obj: &FollowObject, actor_inbox: &String) {
        let mut f = self.followers();
        f.retain(|x| x != &*follow_obj.actor);
        f.push(follow_obj.actor.clone());
        self.write_followers(&f);
        // Send accept to inbox of the actor
        self.post_inbox(
            actor_inbox,
            json!({
                "@context": "https://www.w3.org/ns/activitystreams",
                "id": format!("https://{}/users/{}", self.config.domain, self.config.user),
                "type": "Accept",
                "actor": format!("https://{}/users/{}", self.config.domain, self.config.user),
                "object": follow_obj
            }),
        )
        .await
        .unwrap();
    }

    /**
     * Used by mastodon instances
     */
    fn mastodon_value() -> Value {
        json!({
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
        })
    }

    /**
     * Post JSON Value to a follower
     */
    pub async fn post_inbox(
        &self,
        actor_inbox: &String,
        body: Value,
    ) -> Result<(), reqwest::Error> {
        let client = reqwest::Client::builder()
            .connection_verbose(true)
            .build()?;

        let config = SigningConfig::new(
            &*format!(
                "https://{}/users/{}#main-key",
                self.config.domain, self.config.user
            ),
            RsaSha256Sign::new_pem(&*fs::read(self.config.private_key.clone()).unwrap()).unwrap(),
        );
        println!("Send Value to inbox: {}", actor_inbox);

        let req = client
            .post(actor_inbox)
            .json(&body)
            .header(reqwest::header::ACCEPT, "application/activity+json")
            .build()?
            .signed(&config)
            .unwrap();

        let result = client.execute(req).await?.text().await?;
        println!("=>{}", result);
        Ok(())
    }

    /**
     * @return if we follow somebody
     */
    pub fn is_following(&self, actor: &String) -> bool {
        self.following.contains(actor) || self.pending_following.contains(actor)
    }

    /**
     * Send a Follow Request to an actor, and add it to the pending_following list
     * @param self
     * @param actor
     * @param actor_inbox
     */
    pub async fn send_follow(&mut self, actor: &String, actor_inbox: &String) {
        if !self.is_following(actor) {
            let random_string: String = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(32)
                .map(char::from)
                .collect();

            let follow_obj = json!({
                "@context":"https://www.w3.org/ns/activitystreams",
                "type":"Follow",
                "actor": format!("https://{}/users/{}", self.config.domain, self.config.user),
                "object": actor,
                "id": format!("https://{}/{}", self.config.domain, random_string),
            });
            println!("Send Follow Activity to {}", actor);
            // Send Follow to inbox of the actor
            self.post_inbox(actor_inbox, follow_obj).await.unwrap();
            self.pending_following.push(actor.to_string());
            self.update_pending_following();
        }
    }

    /**
     * Move an object to the confirmed following list
     * @param self
     * @param object
     */
    pub async fn follow_accepted(&mut self, object: &String) {
        if self.pending_following.contains(object) {
            self.pending_following.retain(|x| x != &*object);
            self.following.push(object.to_string());
            self.update_pending_following();
            self.update_following();
            println!("Now following {}", object);
        } else {
            println!("Follow accepted, but pending not found: {}", object);
        }
    }

    /**
     * Get inbox from fediverse actor
     */
    pub async fn get_inbox(actor: &String, prefer_shared: bool) -> Result<String, reqwest::Error> {
        let client = reqwest::Client::new();
        let body = client
            .get(actor)
            .header(reqwest::header::ACCEPT, "application/activity+json")
            .send()
            .await?
            .text()
            .await?;
        let object: Value = serde_json::from_str(&body).unwrap();
        if prefer_shared {
            let inbox = object["endpoints"].as_object();
            if inbox.is_some() {
                let inbox = inbox.unwrap()["sharedInbox"].as_str();
                if inbox.is_some() {
                    return Ok(inbox.unwrap().to_owned());
                }
            }
        }
        Ok(object["inbox"].as_str().unwrap().to_owned())
    }

    /**
     * Get best name from fediverse actor
     */
    pub async fn get_best_name(actor: &String) -> Result<String, reqwest::Error> {
        let client = reqwest::Client::new();
        let body = client
            .get(actor)
            .header(reqwest::header::ACCEPT, "application/activity+json")
            .send()
            .await?
            .text()
            .await?;
        let object: Value = serde_json::from_str(&body).unwrap();
        Ok(object["name"].as_str().unwrap().to_owned())
    }

    /**
     * Follow new instances or manually added contacts from the config files
     * @param self
     */
    pub async fn update_cache(&mut self) {
        // Read from instances.txt
        // If not in followers, send_follow
        if Path::new(&self.config.instances_list).exists() {
            let file = File::open(&*self.config.instances_list).unwrap();
            let reader = BufReader::new(file);
            for actor in reader.lines() {
                let actor = actor.unwrap();
                if !self.is_following(&actor) && !actor.contains(&self.config.domain) {
                    println!("New instance detected: {}", actor);
                    let inbox = Followers::get_inbox(&actor, false)
                        .await
                        .unwrap_or(String::new());
                    if !inbox.is_empty() {
                        self.send_follow(&actor, &inbox).await;
                    }
                }
            }
        }

        // Read from manual
        if Path::new(&self.config.manual_follow_list).exists() {
            let file = File::open(&*self.config.manual_follow_list).unwrap();
            let reader = BufReader::new(file);
            for actor in reader.lines() {
                let actor = actor.unwrap();
                if !self.is_following(&actor) && !actor.contains(&self.config.domain) {
                    println!("Manual follow: {}", actor);
                    let inbox = Followers::get_inbox(&actor, false)
                        .await
                        .unwrap_or(String::new());
                    if !inbox.is_empty() {
                        self.send_follow(&actor, &inbox).await;
                    }
                }
            }
            let _ = fs::remove_file(&*self.config.manual_follow_list);
        }
    }

    /**
     * Write self.following in following.json
     */
    fn update_following(&self) {
        std::fs::write(
            format!("{}/following.json", &self.config.cache_dir),
            serde_json::to_string_pretty(&self.following).unwrap(),
        )
        .unwrap();
    }

    /**
     * Write self.following in pending_following.json
     */
    fn update_pending_following(&self) {
        std::fs::write(
            format!("{}/pending_following.json", &self.config.cache_dir),
            serde_json::to_string_pretty(&self.pending_following).unwrap(),
        )
        .unwrap();
    }
}

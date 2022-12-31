/**
 * Copyright (c) 2022, Sébastien Blin <sebastien.blin@enconn.fr>
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
use serde_json::json;
use http_sig::*;
use serde_json::Value;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::prelude::*;
use std::io::BufReader;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Followers {
    pub config: Config
}

impl Followers {
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
        let mut followers : Vec<String> = Vec::new();
        if Path::new(&*format!("{}/followers", self.config.cache_dir)).exists() {
            let file = File::open(format!("{}/followers", self.config.cache_dir)).unwrap();
            let buf = BufReader::new(file);
            followers = buf.lines()
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
        for follower in followers  {
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
    pub async fn accept(&self, follow_obj: FollowObject, actor_inbox: String) {
        let mut f = self.followers();
        f.retain(|x| x != &*follow_obj.actor);
        f.push(follow_obj.actor.clone());
        self.write_followers(&f);
        // Send accept to inbox of the actor
        self.post_inbox(&actor_inbox, json!({
            "@context": "https://www.w3.org/ns/activitystreams",
            "id": format!("https://{}/users/{}", self.config.domain, self.config.user),
            "type": "Accept",
            "actor": format!("https://{}/users/{}", self.config.domain, self.config.user),
            "object": follow_obj
        })).await.unwrap();
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
    pub async fn post_inbox(&self, actor_inbox: &String, body: Value) -> Result<(), reqwest::Error> {
        let client = reqwest::Client::builder()
            .connection_verbose(true)
            .build()?;

        let config = SigningConfig::new(
                &*format!("https://{}/users/{}#main-key", self.config.domain, self.config.user),
                RsaSha256Sign::new_pem(&*fs::read(self.config.private_key.clone()).unwrap()).unwrap()
            );
        println!("Send Value to inbox: {}", actor_inbox);

        let req = client
            .post(actor_inbox).json(&body)
            .header(reqwest::header::ACCEPT, "application/activity+json")
            .build()?
            .signed(&config).unwrap();

        let result = client.execute(req).await?.text().await?;
        println!("=>{}", result);
        Ok(())
    }

}
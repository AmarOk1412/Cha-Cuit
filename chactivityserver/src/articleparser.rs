/**
 * Copyright (c) 2023, Sébastien Blin <sebastien.blin@enconn.fr>
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

use rand::{distributions::Alphanumeric, Rng};
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use url::Url;

#[derive(Debug, Clone)]
pub struct ArticleParser {
    pub config: Config,
    pub articles: HashMap<String, String>,
}

const TITLE_HEADER: &str = r#"title: ([^\n]+)"#;
const DATE_HEADER: &str = r#"date: ([^\n]+)"#;
const DURATION_HEADER: &str = r#"duration: ([^\n]+)"#;
const TAGS_HEADER: &str = r#"tags: ([^\n]+)"#;
const THUMBNAIL_HEADER: &str = r#"thumbnail: ([^\n]+)"#;

impl ArticleParser {
    pub fn new(config: Config) -> Self {
        let mut articles = HashMap::new();
        let path = format!("{}/articles.json", config.cache_dir);
        if Path::new(&path).exists() {
            articles =
                serde_json::from_str(&*fs::read_to_string(path).unwrap_or(String::new())).unwrap();
        }
        Self { config, articles }
    }

    /**
     * Remove all articles from an user
     * @param self
     * @param actors        Banned actors
     */
    pub fn clear_user(&mut self, actors: &Vec<String>) {
        let output_dir = self.config.output_dir.clone();
        for actor in actors.iter() {
            self.articles.retain(|k, v| {
                let _ = fs::remove_file(format!("{}/{}.md", output_dir, v));
                println!("Removed {}", v);
                k.contains(actor)
            });
        }
        self.update_articles();
    }

    pub fn parse(&mut self, body: Value, best_name: String) {
        // Check that we have title/date/duration
        let mut content = body
            .get("content")
            .unwrap()
            .as_str()
            .unwrap_or("")
            .to_owned();
        let author = body.get("attributedTo").unwrap().as_str().unwrap_or("");
        if content.find("---") != Some(0) || author.len() == 0 {
            return;
        }
        content = (&content[3..]).to_owned();
        let re_title = Regex::new(TITLE_HEADER).unwrap();
        let re_date = Regex::new(DATE_HEADER).unwrap();
        let re_duration = Regex::new(DURATION_HEADER).unwrap();

        let match_title = re_title.captures(&content).unwrap();
        let title = match_title.get(1).map_or("", |m| m.as_str()).to_owned();
        let match_date = re_date.captures(&content).unwrap();
        let date = match_date.get(1).map_or("", |m| m.as_str()).to_owned();
        let match_duration = re_duration.captures(&content).unwrap();
        let duration = match_duration.get(1).map_or("", |m| m.as_str()).to_owned();
        let id = body.get("id").unwrap().as_str().unwrap_or("").to_owned();

        if title.len() == 0
            || date.len() == 0
            || duration.len() == 0
            || id.len() == 0
            || self.articles.contains_key(&id)
        {
            return;
        }

        // Check optional tags + thumbnail
        let re_tags = Regex::new(TAGS_HEADER).unwrap();
        let re_thumbnail = Regex::new(THUMBNAIL_HEADER).unwrap();
        let match_tags = re_tags.captures(&content).unwrap();
        let mut tags = match_tags.get(1).map_or("", |m| m.as_str()).to_owned();
        if tags.len() != 0 {
            tags = format!("tags: {}\n", tags);
        }
        let match_thumbnail = re_thumbnail.captures(&content);
        let mut thumbnail = String::new();
        if match_thumbnail.is_some() {
            thumbnail = match_thumbnail
                .unwrap()
                .get(1)
                .map_or("", |m| m.as_str())
                .to_owned();
            if thumbnail.len() != 0 {
                thumbnail = format!("thumbnail: {}\n", thumbnail);
            }
        }

        let sep = content.find("---");
        if !sep.is_some() {
            return;
        }
        content = (&content[(sep.unwrap() + 4)..]).to_owned();

        content = format!(
            r#"---
title: {}
date: {}
duration: {}
author: {}
{}{}---

{}

{}
"#,
            title, date, duration, best_name, tags, thumbnail, content, id
        );

        // Fix thumbnail/images links (with incoming domain)
        let parsed = Url::parse(author).unwrap();
        let domain = format!("https://{}", parsed.domain().unwrap());
        content = content.replace("](/", &*format!("]({}/", domain));
        content = content.replace("thumbnail: \"/", &*format!("thumbnail: \"{}/", domain));

        // save it (with random id)
        let random_string: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();
        let ret = fs::write(
            format!("{}/{}.md", self.config.output_dir, random_string),
            content,
        );
        if ret.is_ok() {
            self.articles.insert(id, random_string);
            self.update_articles();
            println!("Write a note for {}", title);
        }
    }

    /**
     * Write self.articles in articles.json
     */
    fn update_articles(&self) {
        std::fs::write(
            format!("{}/articles.json", &self.config.cache_dir),
            serde_json::to_string_pretty(&self.articles).unwrap(),
        )
        .unwrap();
    }
}

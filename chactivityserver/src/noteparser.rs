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

use chrono::offset::Utc;
use chrono::DateTime;
use rand::{distributions::Alphanumeric, Rng};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct NoteParser {
    pub config: Config,
    pub notes: HashMap<String, String>,
}

impl NoteParser {
    pub fn new(config: Config) -> Self {
        let mut notes = HashMap::new();
        let path = format!("{}/notes.json", config.cache_dir);
        if Path::new(&path).exists() {
            notes =
                serde_json::from_str(&*fs::read_to_string(path).unwrap_or(String::new())).unwrap();
        }
        Self { config, notes }
    }

    pub fn parse(&mut self, body: Value, best_name: String) {
        // Check that we have valid tag in config (#chacuit)
        let mut tags = String::new();
        let mut idx = 0;
        let mut chacuit_tag_found = false;
        let id = body.get("id").unwrap().as_str().unwrap().to_owned();
        if id.len() == 0 || self.notes.contains_key(&id) {
            return;
        }
        let note_tags = body.get("tag").unwrap().as_array().unwrap();
        for nt in note_tags.iter() {
            let nt = nt
                .get("name")
                .unwrap()
                .as_str()
                .unwrap_or("")
                .replace("#", "");
            if nt == "chacuit" {
                chacuit_tag_found = true;
            }
            if idx > 0 {
                tags += ", ";
            }
            idx += 1;
            tags += &*format!("\"{}\"", nt);
        }
        let mut gallery = String::new();
        let mut thumbnail = String::new();
        let note_attachment = body.get("attachment").unwrap().as_array().unwrap();
        for att in note_attachment.iter() {
            let att = att.get("url").unwrap().as_str().unwrap_or("");
            if thumbnail.len() == 0 {
                thumbnail = format!("thumbnail: \"{}\"\n", att);
            }
            gallery += &*format!("{{{{< figure src=\"{}\" >}}}}\n", att);
        }
        if gallery.len() > 0 {
            gallery = format!("{{{{< gallery >}}}}\n{}{{{{< /gallery >}}}}", gallery);
        }

        if !chacuit_tag_found {
            return;
        }

        // Build file:
        let title = body.get("summary").unwrap().as_str().unwrap_or("");
        let html_content = body.get("content").unwrap().as_str().unwrap_or("");
        let content = html2text::from_read(&html_content.as_bytes()[..], html_content.len());
        if title.len() == 0 || content.len() == 0 {
            return;
        }

        let now = SystemTime::now();
        let datetime: DateTime<Utc> = now.into();
        let published = datetime.format("%Y-%m-%d").to_string();

        let content = format!(
            r#"
---
title: {}
date: {}
tags: [{}]
author: {}
{}---

{}

{}

{}
"#,
            title, published, tags, best_name, thumbnail, content, gallery, id
        );

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
            self.notes.insert(id, random_string);
            self.update_notes();
            println!("Write a note for {}", title);
        }
    }

    /**
     * Write self.notes in notes.json
     */
    fn update_notes(&self) {
        std::fs::write(
            format!("{}/notes.json", &self.config.cache_dir),
            serde_json::to_string_pretty(&self.notes).unwrap(),
        )
        .unwrap();
    }
}

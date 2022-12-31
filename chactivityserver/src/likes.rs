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

use serde_json::{json, Value};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Likes {
    pub config: Config,
    pub data: Value,
}

impl Likes {

    /**
     * This structure is used to store the boost/likes per recipes
     * data is a json like:
     * {
     *     "/recettes/profiteroles-chocolat": {
     *         "boost": [],
     *         "like": []
     *     }
     * }
     * Data is serialized in .cache/likes.json
     */
    pub fn new(config: Config) -> Likes {
        let mut data = json!({});
        let path = format!("{}/likes.json", config.cache_dir);
        if Path::new(&path).exists() {
            data = serde_json::from_str(&*fs::read_to_string(path).unwrap_or(String::new())).unwrap();
        }

        Likes {
            config,
            data
        }
    }

    /**
     * Add a like
     * @param self
     * @param object
     * @param actor
     */
    pub fn like(&mut self, object: &String, actor: &String) {
        self.update_data(object, actor, &String::from("like"));
    }

    /**
     * Remove a like
     * @param self
     * @param object
     * @param actor
     */
    pub fn unlike(&mut self, object: &String, actor: &String) {
        let mut like = self.data.get_mut(object);
        if !like.is_some() {
            return
        }
        like = like.unwrap().get_mut("like");
        if !like.is_some() {
            return
        }
        let likes = like.unwrap().as_array_mut();
        if !likes.is_some() {
            return
        }
        likes.unwrap().retain(|x| x.as_str().unwrap() != actor);
        self.save_data()
    }

    /**
     * Add a boost
     * @param self
     * @param object
     * @param actor
     */
    pub fn boost(&mut self, object: &String, actor: &String) {
        self.update_data(object, actor, &String::from("boost"));
    }

    /**
     * Remove a boost
     * @param self
     * @param object
     * @param actor
     */
    pub fn unboost(&mut self, object: &String, actor: &String) {
        let mut boost = self.data.get_mut(object);
        if !boost.is_some() {
            return
        }
        boost = boost.unwrap().get_mut("boost");
        if !boost.is_some() {
            return
        }
        let boosts = boost.unwrap().as_array_mut();
        if !boosts.is_some() {
            return
        }
        boosts.unwrap().retain(|x| x.as_str().unwrap() != actor);
        self.save_data()
    }

    /**
     * Get specific data
     * @param self
     * @param object
     * @param wanted_type   Supported: like/boost
     */
    pub fn data(&self, object: &String, wanted_type: &String) -> Value {
        let mut data = self.data.get(object);
        if !data.is_some() {
            return json!([]);
        }
        data = data.unwrap().get(wanted_type);
        if !data.is_some() {
            return json!([]);
        }
        data.unwrap().clone()
    }

    /**
     * Serialize in .cache/likes.json
     */
    fn save_data(&self) {
        std::fs::write(
            format!("{}/likes.json", &self.config.cache_dir),
            serde_json::to_string_pretty(&self.data).unwrap(),
        ).unwrap();
    }

    /**
     * Used by like/boost to update the structure
     */
    fn update_data(&mut self, object: &String, actor: &String, wanted_type: &String) {
        // Ignore profile as status will be relative anyway
        let object = object.replace(&*format!("https://{}", self.config.domain), "");
        // Update data[object][wanted_type][actor];
        let v = Value::String(actor.to_string());
        let mut obj = self.data.get_mut(&*object);
        if obj.is_some() {
            obj = obj.unwrap().get_mut(wanted_type);
            if obj.is_some() {
                if obj.as_ref().unwrap().as_array().is_some() {
                    if obj.as_ref().unwrap().as_array().unwrap().contains(&v) {
                        return;
                    }
                    obj.unwrap().as_array_mut().unwrap().push(v);
                    self.save_data();
                    return;
                }
            }
        }
        self.data[object][wanted_type] = json!([v]);
        self.save_data()
    }

}
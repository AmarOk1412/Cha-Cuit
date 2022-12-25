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

mod config;
mod follow;
mod profile;
mod server;

use crate::config::Config;
use crate::follow::Followers;
use crate::profile::Profile;
use crate::server::Server;

use actix_web::{web, App, HttpServer, web::Data};
use std::sync::Mutex;
use std::fs;

// TODO add logs

fn main() {
    // Init logging
    env_logger::init();
    // Run actix_web with tokio to allow both incoming and outgoing requests
    actix_web::rt::System::with_tokio_rt(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(8)
            .thread_name("main-tokio")
            .build()
            .unwrap()
    })
    .block_on(run_server());
}

async fn run_server() {
    let config_str = fs::read_to_string("config.json") ;
    let config = serde_json::from_str::<Config>(&config_str.unwrap()).unwrap();
    let followers = Followers {
        config: config.clone(),
    };
    let profile = Profile {
        config: config.clone(),
    };
    let server = Data::new(Mutex::new(Server {
        config: config.clone(),
        followers,
        profile
    }));
    HttpServer::new(move || {
        App::new()
            .app_data(server.clone())
            .route("/.well-known/webfinger", web::get().to(Server::webfinger))
            .route("/users/chef", web::get().to(Server::profile))
            .route("/users/chef/inbox", web::post().to(Server::inbox))
            .route("/users/chef/outbox", web::get().to(Server::outbox))
            .route("/users/chef/followers", web::get().to(Server::user_followers))
    })
    .bind(&*config.bind_address).unwrap()
    .run()
    .await
    .unwrap()
}
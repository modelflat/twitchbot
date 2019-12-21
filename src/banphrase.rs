use std::future::Future;

use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct BanphraseRequest {
    message: String,
}

#[derive(Deserialize)]
pub struct BanphraseResponse {
    pub banned: bool,
}

pub struct BanphraseAPI {
    session: Client,
    url: String,
}

impl BanphraseAPI {
    pub fn new(url: String) -> BanphraseAPI {
        BanphraseAPI {
            session: Client::new(),
            url,
        }
    }

    pub fn check(&self, message: String) -> impl Future<Output = reqwest::Result<Response>> {
        self.session.post(&self.url).json(&BanphraseRequest { message }).send()
    }
}

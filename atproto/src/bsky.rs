/* A lot of this is cribbed from bisky https://github.com/jesopo/bisky/
 * but adjusted to meet the very specific requirements of this blog.
 *
 * BiSky License:

MIT License

Copyright (c) 2023 jesopo

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
 */

use anyhow::bail;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::json;

pub struct BSky {
    session: Session,
    service_url: reqwest::Url,
}

#[derive(Deserialize, Serialize)]
struct Session {
    pub did: String,
    pub email: String,
    pub handle: String,
    #[serde(rename(deserialize = "accessJwt"))]
    pub access_jwt: String,
    #[serde(rename(deserialize = "refreshJwt"))]
    pub refresh_jwt: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ExternalObject {
    pub uri: String,
    pub title: String,
    pub description: String,
    #[serde(rename(deserialize = "maxSize", serialize = "maxSize"))]
    pub max_size: Option<usize>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct External {
    pub external: ExternalObject,
}

impl External {
    pub fn new(uri: String, title: String, description: String, max_size: Option<usize>) -> Self {
        Self {
            external: ExternalObject {
                uri,
                title,
                description,
                max_size,
            },
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "$type")]
pub enum Embeds {
    #[serde(rename(
        deserialize = "app.bsky.embed.external",
        serialize = "app.bsky.embed.external"
    ))]
    External(External),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Post {
    #[serde(rename(deserialize = "createdAt", serialize = "createdAt"))]
    pub created_at: DateTime<Utc>,
    #[serde(rename(deserialize = "$type", serialize = "$type"))]
    pub rust_type: Option<String>,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embed: Option<Embeds>,
}

impl Post {
    pub fn new(text: String, embed: Option<Embeds>) -> Self {
        Self {
            created_at: chrono::offset::Utc::now(),
            text,
            embed,
            rust_type: None,
        }
    }
}

#[derive(Serialize)]
pub struct CreateRecord<'a, T> {
    pub repo: &'a str,
    pub collection: &'a str,
    pub record: T,
}

#[derive(Debug, Deserialize)]
pub struct CreateRecordOutput {
    pub cid: String,
    pub uri: String,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    pub error: String,
    pub message: String,
}

impl BSky {
    pub async fn login(
        service_url: &reqwest::Url,
        identifier: &str,
        password: &str,
    ) -> Result<Self, anyhow::Error> {
        let response = reqwest::Client::new()
            .post(service_url.join("xrpc/com.atproto.server.createSession")?)
            .header("content-type", "application/json")
            .body(
                json!({
                    "identifier": identifier,
                    "password": password,
                })
                .to_string(),
            )
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            bail!("BlueSky credentials unauthorised")
        } else if response.status() == reqwest::StatusCode::BAD_REQUEST {
            let error_details = response.json::<ApiError>().await?;
            bail!("API Error {:?}", error_details)
        } else {
            let user_session = response.json::<Session>().await?;
            Ok(BSky {
                session: user_session,
                service_url: service_url.clone(),
            })
        }
    }

    pub async fn new_post(&self, post: Post) -> Result<(), anyhow::Error> {
        self.repo_create_record::<CreateRecordOutput, Post>(
            &self.session.handle,
            "app.bsky.feed.post",
            post,
        )
        .await?;

        Ok(())
    }

    async fn repo_create_record<D: DeserializeOwned, S: Serialize>(
        &self,
        repo: &str,
        collection: &str,
        record: S,
    ) -> Result<D, anyhow::Error> {
        self.xrpc_post(
            "com.atproto.repo.createRecord",
            &CreateRecord {
                repo,
                collection,
                record,
            },
        )
        .await
    }

    async fn xrpc_post<D1: Serialize, D2: DeserializeOwned>(
        &self,
        path: &str,
        body: &D1,
    ) -> Result<D2, anyhow::Error> {
        let body = serde_json::to_string(body)?;

        let req = reqwest::Client::new()
            .post(self.service_url.join(&format!("xrpc/{path}")).unwrap())
            .header("content-type", "application/json")
            .header(
                "authorization",
                format!("Bearer {}", self.session.access_jwt),
            )
            .body(body.to_string());

        let response = req.send().await?;

        if response.status() == reqwest::StatusCode::BAD_REQUEST {
            let error = response.json::<ApiError>().await?;
            bail!("Api Error {:?}", error);
        } else {
            let text = response.error_for_status()?.text().await?;
            Ok(serde_json::from_str(&text)?)
        }
    }
}

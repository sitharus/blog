use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::settings::Settings;

pub const PUBLIC_TIMELINE: &str = "https://www.w3.org/ns/activitystreams#Public";

#[derive(Serialize, Debug, PartialEq, Clone)]
#[serde(into = "OrderedCollectionJsonLD<T>")]
pub struct OrderedCollection<T>
where
    T: Serialize + Clone,
{
    pub id: Option<String>,
    pub summary: Option<String>,
    pub items: Vec<T>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct OrderedCollectionJsonLD<T>
where
    T: Serialize + Clone,
{
    #[serde(rename = "@context")]
    context: String,
    summary: Option<String>,
    #[serde(rename = "type")]
    collection_type: String,
    total_items: usize,
    ordered_items: Vec<T>,
}

impl<T> From<OrderedCollection<T>> for OrderedCollectionJsonLD<T>
where
    T: Serialize + Clone,
{
    fn from(val: OrderedCollection<T>) -> Self {
        OrderedCollectionJsonLD {
            context: "https://www.w3.org/ns/activitystreams".into(),
            summary: val.summary,
            collection_type: "OrderedCollection".into(),
            total_items: val.items.len(),
            ordered_items: val.items,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum Activity {
    Note(Box<Note>),
    Follow(Box<Follow>),
    Create(Box<Create>),
    Undo(Box<Undo>),
    Delete(Box<Delete>),
    Like(Box<Like>),
    Update(Box<Update>),
    Person(Box<Actor>),
}

impl Activity {
    pub fn create(actor: String, object: Activity, to: Vec<String>, cc: Vec<String>) -> Self {
        Self::Create(Box::new(Create::new(actor, object, to, cc)))
    }

    pub fn note(
        content: String,
        id: String,
        published: chrono::DateTime<Utc>,
        to: Vec<String>,
        cc: Vec<String>,
    ) -> Self {
        Self::Note(Box::new(Note::new(content, id, published, to, cc)))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Context {
    String(String),
    List(Vec<serde_json::Value>),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Create {
    #[serde(rename = "@context")]
    context: Context,
    pub actor: String,
    pub published: chrono::DateTime<Utc>,
    to: Vec<String>,
    cc: Vec<String>,
    object: Box<Activity>,
}

impl Create {
    pub fn new(actor: String, object: Activity, to: Vec<String>, cc: Vec<String>) -> Create {
        Self {
            context: Context::String("https://www.w3.org/ns/activitystreams".into()),
            actor,
            object: Box::new(object),
            published: chrono::Utc::now(),
            to,
            cc,
        }
    }

    pub fn object(&self) -> &Activity {
        &self.object
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Note {
    #[serde(rename = "@context")]
    context: Option<String>,
    pub content: String,
    pub id: String,
    pub published: chrono::DateTime<Utc>,
    to: Vec<String>,
    cc: Vec<String>,
}

impl Note {
    pub fn new(
        content: String,
        id: String,
        published: chrono::DateTime<Utc>,
        to: Vec<String>,
        cc: Vec<String>,
    ) -> Self {
        Note {
            context: Some("https://www.w3.org/ns/activitystreams".into()),
            content,
            id,
            published,
            to,
            cc,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Accept {
    #[serde(rename = "@context")]
    context: String,
    #[serde(rename = "type")]
    activity_type: String,
    pub object: Activity,
    pub actor: String,
}

impl Accept {
    fn new(actor: String, accepting: Activity) -> Accept {
        Accept {
            context: "https://www.w3.org/ns/activitystreams".into(),
            activity_type: "Accept".into(),
            object: accepting,
            actor,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Follow {
    pub object: String,
    pub actor: String,
    pub id: String,
}

impl Follow {
    pub fn accept(&self, by: String) -> Accept {
        Accept::new(by, Activity::Follow(Box::new(self.clone())))
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Delete {
    pub object: String,
    pub actor: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Undo {
    #[serde(rename = "@context")]
    context: String,
    actor: String,
    pub object: Box<Activity>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Like {
    pub actor: String,
    pub object: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Update {
    #[serde(rename = "@context")]
    context: String,
    pub actor: String,
    pub id: String,
    pub object: Box<Activity>,
    to: Vec<String>,
    cc: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Actor {
    #[serde(rename = "@context")]
    context: Vec<String>,
    id: String,
    preferred_username: String,
    inbox: String,
    outbox: String,
    followers: String,
    following: String,
    public_key: PublicKey,
    name: String,
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon: Option<MediaRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image: Option<MediaRef>,
    published: DateTime<Utc>,
}

impl Actor {
    pub fn new(settings: Settings) -> Actor {
        let fedi_base = settings.activitypub_base();
        let actor_name = settings.actor_name.clone();
        let id = settings.activitypub_actor_uri();
        let owner_id = settings.activitypub_actor_uri();
        let key_id = settings.activitypub_key_id();
        Actor {
            context: vec![
                "https://www.w3.org/ns/activitystreams".into(),
                "https://w3id.org/security/v1".into(),
            ],
            id,
            preferred_username: actor_name,
            inbox: format!("{}inbox", fedi_base),
            outbox: format!("{}outbox", fedi_base),
            followers: format!("{}followers", fedi_base),
            following: format!("{}following", fedi_base),
            public_key: PublicKey {
                id: key_id,
                owner: owner_id,
                public_key_pem: settings.fedi_public_key_pem,
            },
            name: settings.blog_name,
            url: settings.base_url.clone(),
            icon: settings
                .fedi_avatar
                .map(|a| as_media_ref(&a, &settings.base_url, &settings.media_base_url)),
            image: settings
                .fedi_header
                .map(|a| as_media_ref(&a, &settings.base_url, &settings.media_base_url)),
            published: settings.profile_last_updated,
        }
    }
}

fn as_media_ref(media: &str, base_url: &str, media_base_url: &str) -> MediaRef {
    let url = Url::parse(base_url)
        .unwrap()
        .join(media_base_url)
        .unwrap()
        .join(media)
        .unwrap()
        .to_string();

    MediaRef {
        url,
        media_type: if media.ends_with("png") {
            "image/png"
        } else {
            "image/jpeg"
        }
        .into(),
        item_type: MediaRefType::Image,
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MediaRefType {
    Image,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct PublicKey {
    id: String,
    owner: String,
    public_key_pem: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MediaRef {
    url: String,
    #[serde(rename = "type")]
    item_type: MediaRefType,
    media_type: String,
}

impl Update {
    pub fn new(
        actor: String,
        id: String,
        object: Activity,
        to: Vec<String>,
        cc: Vec<String>,
    ) -> Update {
        Update {
            context: "https://www.w3.org/ns/activitystreams".into(),
            actor,
            id,
            object: Box::new(object),
            to,
            cc,
        }
    }
}

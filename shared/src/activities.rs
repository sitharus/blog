use chrono::Utc;
use serde::{Deserialize, Serialize};

pub const PUBLIC_TIMELINE: &str = "https://www.w3.org/ns/activitystreams#Public";

#[derive(Serialize, Debug, PartialEq, Clone)]
#[serde(into = "OrderedCollectionJsonLD<T>")]
pub struct OrderedCollection<T>
where
    T: Serialize + Clone,
{
    pub summary: Option<String>,
    pub items: Vec<T>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct OrderedCollectionJsonLD<T>
where
    T: Serialize + Clone,
{
    context: String,
    summary: Option<String>,
    #[serde(rename = "type")]
    collection_type: String,
    total_items: usize,
    ordered_items: Vec<T>,
}

impl<T> Into<OrderedCollectionJsonLD<T>> for OrderedCollection<T>
where
    T: Serialize + Clone,
{
    fn into(self) -> OrderedCollectionJsonLD<T> {
        OrderedCollectionJsonLD {
            context: "https://www.w3.org/ns/activitystreams".into(),
            summary: self.summary,
            collection_type: "OrderedCollection".into(),
            total_items: self.items.len(),
            ordered_items: self.items,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum Activity {
    Note(Note),
    Follow(Follow),
    Create(Create),
}

impl Activity {
    pub fn create(actor: String, object: Activity, to: Vec<String>, cc: Vec<String>) -> Self {
        Self::Create(Create::new(actor, object, to, cc))
    }

    pub fn note(
        content: String,
        id: String,
        published: chrono::DateTime<Utc>,
        to: Vec<String>,
        cc: Vec<String>,
    ) -> Self {
        Self::Note(Note::new(content, id, published, to, cc))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Create {
    #[serde(rename = "@context")]
    context: String,
    actor: String,
    published: chrono::DateTime<Utc>,
    to: Vec<String>,
    cc: Vec<String>,
    object: Box<Activity>,
}

impl Create {
    fn new(actor: String, object: Activity, to: Vec<String>, cc: Vec<String>) -> Create {
        Self {
            context: "https://www.w3.org/ns/activitystreams".into(),
            actor,
            object: Box::new(object),
            published: chrono::Utc::now(),
            to,
            cc,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Note {
    #[serde(rename = "@context")]
    context: String,
    content: String,
    id: String,
    published: chrono::DateTime<Utc>,
    to: Vec<String>,
    cc: Vec<String>,
}

impl Note {
    fn new(
        content: String,
        id: String,
        published: chrono::DateTime<Utc>,
        to: Vec<String>,
        cc: Vec<String>,
    ) -> Self {
        Note {
            context: "https://www.w3.org/ns/activitystreams".into(),
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
        Accept::new(by, Activity::Follow(self.clone()))
    }
}

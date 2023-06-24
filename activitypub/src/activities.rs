use serde::{Deserialize, Serialize};

#[derive(Serialize, Debug, PartialEq, Clone)]
#[serde(into = "OrderedCollectionJsonLD<T>")]
pub struct OrderedCollection<T>
where
    T: Serialize + Clone,
{
    pub summary: String,
    pub items: Vec<T>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct OrderedCollectionJsonLD<T>
where
    T: Serialize + Clone,
{
    context: String,
    summary: String,
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
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Note {
    pub name: String,
    pub content: String,
    pub id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BasicActor {
    #[serde(rename = "type")]
    actor_type: String,
    name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Accept {
    #[serde(rename = "context")]
    context: String,
    #[serde(rename = "type")]
    activity_type: String,
    pub object: Activity,
    pub actor: BasicActor,
}

impl Accept {
    fn new(actor: String, accepting: Activity) -> Accept {
        Accept {
            context: "https://www.w3.org/ns/activitystreams".into(),
            activity_type: "Accept".into(),
            object: accepting,
            actor: BasicActor {
                actor_type: "Person".into(),
                name: actor,
            },
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

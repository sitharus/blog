use serde::Serialize;

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

#[derive(Serialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum Activity {
    Note(Note),
}

#[derive(Serialize, Debug, Clone)]
pub struct Note {
    pub name: String,
    pub content: String,
    pub id: String,
}

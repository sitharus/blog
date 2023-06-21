use serde::Serialize;
use shared::settings::Settings;

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Actor {
    #[serde(rename = "@context")]
    context: Vec<String>,
    id: String,
    #[serde(rename = "type")]
    actor_type: String,
    preferred_username: String,
    inbox: String,
    outbox: String,
    followers: String,
    following: String,
    public_key: PublicKey,
    name: String,
    url: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct PublicKey {
    id: String,
    owner: String,
    public_key_pem: String,
}

impl Actor {
    pub fn new(
        fedi_base: String,
        actor_name: String,
        username: String,
        settings: Settings,
    ) -> Actor {
        let id = format!("{}{}", fedi_base, actor_name);
        let owner_id = id.clone();
        let key_id = format!("{}{}#main-key", fedi_base, actor_name);
        Actor {
            context: vec![
                "https://www.w3.org/ns/activitystreams".into(),
                "https://w3id.org/security/v1".into(),
            ],
            id,
            actor_type: "Person".into(),
            preferred_username: username,
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
            url: settings.base_url,
        }
    }
}

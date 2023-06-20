use serde::Serialize;

#[derive(Serialize, Debug, Clone)]
pub struct Finger {
    subject: String,
    links: Vec<Link>,
}

#[derive(Serialize, Debug, Clone)]
struct Link {
    rel: String,
    #[serde(rename = "type")]
    link_type: String,
    href: String,
}

impl Finger {
    pub fn new<T: ToString>(user: T, host: T, actor_uri: T) -> Finger {
        let subject = format!("acct:{}@{}", user.to_string(), host.to_string());
        let links = [Link {
            rel: "self".into(),
            link_type: "application/activity+json".into(),
            href: actor_uri.to_string(),
        }]
        .to_vec();
        Finger { subject, links }
    }
}

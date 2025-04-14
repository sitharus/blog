use anyhow::{anyhow, bail};
use base64::{engine::general_purpose, Engine as _};
use cgi::http::header;
use rsa::{
    pkcs1::{DecodeRsaPrivateKey, DecodeRsaPublicKey},
    pkcs1v15::{self, SigningKey, VerifyingKey},
    sha2::{Digest, Sha256},
    signature::{RandomizedSigner, SignatureEncoding, Verifier},
    RsaPrivateKey,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shared::settings::Settings;
use sqlx::{query, PgPool};
use ureq::{Request, Response};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignatureHeader {
    key_id: String,
    algorithm: String,
    headers: Option<String>,
    signature: String,
}

pub async fn validate(
    request: &cgi::Request,
    connection: &PgPool,
    settings: &Settings,
) -> anyhow::Result<String> {
    let signature = request.headers().get("Signature");
    let digest = request.headers().get("Digest");
    match signature {
        Some(signature) => {
            let sig: SignatureHeader = serde_querystring::from_bytes(
                signature.as_bytes(),
                serde_querystring::ParseMode::UrlEncoded,
            )?;

            if sig.algorithm != "rsa-sha256" {
                bail!("Algorithm {} not supported", sig.algorithm);
            }
            let digest_str: Option<String> =
                digest.and_then(|d| d.to_str().ok()).map(|s| s.to_string());

            let computed_digest = match digest {
                Some(_) => {
                    let body = request.body();
                    let digest = Sha256::digest(body);
                    Some(format!(
                        "SHA-256={}",
                        general_purpose::STANDARD.encode(digest)
                    ))
                }
                None => None,
            };
            if computed_digest != digest_str {
                bail!("Digest does not match")
            }

            let public_key =
                get_or_update_actor_public_key(&sig.key_id, connection, settings).await?;
            let verifying_key = VerifyingKey::<Sha256>::from_pkcs1_pem(&public_key)?;

            let signature_parts: Vec<String> = sig
                .headers
                .unwrap_or("date".to_string())
                .split(" ")
                .map(|header| match header.to_ascii_lowercase().as_str() {
                    "(request-target)" => format!(
                        "{}: {} {}",
                        header,
                        request.method().as_str(),
                        request.uri().path()
                    ),
                    "digest" => format!("{}: {}", header, digest_str.clone().unwrap_or_default()),

                    _ => {
                        let header_text = request.headers().get(header);
                        format!(
                            "{}: {}",
                            header,
                            header_text
                                .and_then(|f| f.to_str().ok())
                                .unwrap_or_default()
                        )
                    }
                })
                .collect();
            let signing_string = signature_parts.join("\n");
            let decoded_signature: pkcs1v15::Signature = general_purpose::STANDARD
                .decode(sig.signature)?
                .as_slice()
                .try_into()?;

            verifying_key.verify(signing_string.as_bytes(), &decoded_signature)?;
            Ok(sig.key_id)
        }
        _ => bail!("Signature not present"),
    }
}

pub fn sign_and_send<T>(request: Request, body: T, settings: &Settings) -> anyhow::Result<Response>
where
    T: Serialize,
{
    let body = serde_json::to_vec(&body)?;
    let mut rng = rand::thread_rng();

    let date = chrono::Utc::now()
        .format("%a, %d %b %Y %H:%M:%S GMT")
        .to_string();
    let request_url = request.request_url()?;

    let host = request_url.host();
    let path = request_url.path();
    let method = request.method().to_lowercase();

    let private_key = RsaPrivateKey::from_pkcs1_pem(&settings.fedi_private_key_pem)?;
    let signing_key = SigningKey::<Sha256>::new(private_key);

    let body_digest = Sha256::digest(&body);
    let digest_header = format!("SHA-256={}", general_purpose::STANDARD.encode(body_digest));

    let signature_string = format!(
        "(request-target): {} {}\nhost: {}\ndate: {}\ndigest: {}",
        method, path, host, date, digest_header
    );
    let signature = signing_key.sign_with_rng(&mut rng, signature_string.as_bytes());
    let b64_sig = general_purpose::STANDARD.encode(signature.to_bytes());

    let signature_header = format!(
        "keyId=\"{}\",algorithm=\"rsa-sha256\",headers=\"(request-target) host date digest\",signature=\"{}\"",
        settings.activitypub_key_id(),
        b64_sig
    );

    Ok(request
        .set(header::DATE.as_str(), &date.to_owned())
        .set("Signature", &signature_header)
        .set("Digest", &digest_header)
        .send(body.as_slice())?)
}

pub fn sign_and_call(request: Request, settings: &Settings) -> anyhow::Result<Response> {
    let mut rng = rand::thread_rng();

    let date = chrono::Utc::now()
        .format("%a, %d %b %Y %H:%M:%S GMT")
        .to_string();
    let request_url = request.request_url()?;

    let host = request_url.host();
    let path = request_url.path();
    let method = request.method().to_lowercase();

    let private_key = RsaPrivateKey::from_pkcs1_pem(&settings.fedi_private_key_pem)?;
    let signing_key = SigningKey::<Sha256>::new(private_key);

    let signature_string = format!(
        "(request-target): {} {}\nhost: {}\ndate: {}",
        method, path, host, date
    );
    let signature = signing_key.sign_with_rng(&mut rng, signature_string.as_bytes());
    let b64_sig = general_purpose::STANDARD.encode(signature.to_bytes());

    let signature_header = format!(
        "keyId=\"{}\",algorithm=\"rsa-sha256\",headers=\"(request-target) host date\",signature=\"{}\"",
        settings.activitypub_key_id(),
        b64_sig
    );

    Ok(request
        .set(header::DATE.as_str(), &date.to_owned())
        .set("Signature", &signature_header)
        .call()?)
}

async fn get_or_update_actor_public_key(
    actor_or_key_id: &str,
    connection: &PgPool,
    settings: &Settings,
) -> anyhow::Result<String> {
    let maybe_existing = query!(
        "SELECT public_key FROm activitypub_known_actors WHERE public_key_id=$1 OR actor=$1",
        actor_or_key_id
    )
    .fetch_optional(connection)
    .await?;
    match maybe_existing {
        Some(existing) if existing.public_key.is_some() => {
            Ok(existing.public_key.unwrap_or_default())
        }
        _ => {
            let actor_details: Value = sign_and_call(
                ureq::get(actor_or_key_id)
                    .set(header::ACCEPT.as_str(), "application/activity+json"),
                settings,
            )?
            .into_json()?;

            let inbox = actor_details["inbox"].as_str().unwrap();

            let result = query!(
                "INSERT INTO activitypub_known_actors(is_following, actor, public_key, inbox, public_key_id) VALUES (true, $1, $2, $3, $4) ON CONFLICT(actor) DO UPDATE SET is_following=true, public_key=$2, public_key_id=$4 RETURNING id, public_key",
                actor_details["id"].as_str(),
                actor_details["publicKey"]["publicKeyPem"].as_str(),
                inbox,
                actor_details["publicKey"]["id"].as_str()
            )
            .fetch_one(connection)
            .await?;

            result.public_key.ok_or(anyhow!("Key not found!"))
        }
    }
}

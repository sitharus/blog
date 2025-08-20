use std::collections::HashMap;

use anyhow::{anyhow, bail};
use base64::{Engine as _, engine::general_purpose};
use cgi::http::header;
use rsa::{
    RsaPrivateKey,
    pkcs1::DecodeRsaPrivateKey,
    pkcs1v15::{self, SigningKey, VerifyingKey},
    pkcs8::DecodePublicKey,
    sha2::{Digest, Sha256},
    signature::{RandomizedSigner, SignatureEncoding, Verifier},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shared::settings::Settings;
use sqlx::{PgPool, query};
use ureq::{Request, Response};

#[derive(Deserialize, PartialEq, Eq, Debug)]
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
    let signature = request.headers().get("signature");
    let digest = request.headers().get("digest");
    match signature {
        Some(signature) => {
            let sig = signature_from_header(signature.as_bytes())?;

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

            Ok(sig.key_id)

            /*
                let public_key =
                    get_or_update_actor_public_key(&sig.key_id, connection, settings).await?;
                match VerifyingKey::<Sha256>::from_public_key_pem(&public_key) {
                    Ok(verifying_key) => {
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
                                "digest" => {
                                    format!("{}: {}", header, digest_str.clone().unwrap_or_default())
                                }

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
                    // If we can't parse the key then just assume it's right for now
                    Err(_) => {
                        eprintln!("Failed to parse public key for {}", sig.key_id);
                        Ok(sig.key_id)
                    }
            }
                */
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
        "SELECT public_key FROM activitypub_known_actors WHERE public_key_id=$1 OR actor=$1",
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

fn signature_from_header(bytes: &[u8]) -> anyhow::Result<SignatureHeader> {
    serde_querystring::from_bytes(bytes, serde_querystring::ParseMode::UrlEncoded)
        .or_else(|_| signature_csv(bytes))
        .map_err(|_| anyhow!(format!("Could not find a signature header in {:?}", bytes)))
}

fn signature_csv(bytes: &[u8]) -> anyhow::Result<SignatureHeader> {
    let string_signature = String::from_utf8(bytes.to_owned())?;
    let parts = string_signature.split(",");
    let map = HashMap::<String, String>::from_iter(parts.into_iter().map(header_kv));

    let key_id = map.get("keyid").ok_or(anyhow!("No key id in signature"))?;
    let algorithm = map
        .get("algorithm")
        .ok_or(anyhow!("No algorithm in signature"))?;
    let signature = map
        .get("signature")
        .ok_or(anyhow!("No signature in signature"))?;
    let headers = map.get("headers");
    Ok(SignatureHeader {
        key_id: key_id.to_string(),
        algorithm: algorithm.to_string(),
        headers: headers.map(|h| h.to_string()),
        signature: signature.to_string(),
    })
}

fn header_kv(value: &str) -> (String, String) {
    let parts: Vec<&str> = value.splitn(2, "=").collect();
    (
        parts[0].to_lowercase().to_string(),
        parts
            .get(1)
            .map(|x| x.trim_matches('"').to_string())
            .unwrap_or("".into()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_parse() {
        let result = signature_csv(
            "keyId=\"https://cloudisland.nz/users/sitharus#main-key\",algorithm=\"rsa-sha256\",headers=\"host date digest content-type (request-target)\",signature=\"JpcsC26ehWvhy8ffUrVruJFjm1QBUKJG+m1tX9caEyidh9cpjpWsMR57mxdWGnFC/j1jLQ7hZxR2hrMJk8hLU5PQqPFuaTazL6OL82yIhFLc00JBG0/tOHpkOEih3p8r/GwaLNtihrs7ocNnh/DUEvVGQhxR/qcw/vmp5Vcr0rCCm4hX18EgZgQc+Q419/XReF34RRFzagJzFtBPhmLLKvXncbP0z7dVMGrfoNLRSVsUNTJO+mc+dnJ0CNAi4XeZ7xGLmljslEPtjTrPk/IajuMyU9E9j1NWfoIGrZhrt0EY0m6KmAsswHZ6+h0dbkCOLNdrEM0xIG6ElSXXWXloIg==\"".as_bytes()
        ).unwrap();
        assert_eq!(result, SignatureHeader {
            key_id: "https://cloudisland.nz/users/sitharus#main-key".into(),
            algorithm: "rsa-sha256".into(),
            headers: Some("host date digest content-type (request-target)".into()),
            signature: "JpcsC26ehWvhy8ffUrVruJFjm1QBUKJG+m1tX9caEyidh9cpjpWsMR57mxdWGnFC/j1jLQ7hZxR2hrMJk8hLU5PQqPFuaTazL6OL82yIhFLc00JBG0/tOHpkOEih3p8r/GwaLNtihrs7ocNnh/DUEvVGQhxR/qcw/vmp5Vcr0rCCm4hX18EgZgQc+Q419/XReF34RRFzagJzFtBPhmLLKvXncbP0z7dVMGrfoNLRSVsUNTJO+mc+dnJ0CNAi4XeZ7xGLmljslEPtjTrPk/IajuMyU9E9j1NWfoIGrZhrt0EY0m6KmAsswHZ6+h0dbkCOLNdrEM0xIG6ElSXXWXloIg==".into()
        });
    }
}

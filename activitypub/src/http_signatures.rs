use base64::{engine::general_purpose, Engine as _};
use cgi::http::header;
use rand;
use rsa::{
    pkcs1::DecodeRsaPrivateKey,
    pkcs1v15::SigningKey,
    sha2::{Digest, Sha256},
    signature::{RandomizedSigner, SignatureEncoding},
    RsaPrivateKey,
};
use serde::Serialize;
use shared::settings::Settings;
use ureq::{Request, Response};

pub async fn validate(request: &cgi::Request) -> anyhow::Result<bool> {
    Ok(true)
}

pub fn sign_and_send<T>(request: Request, body: T, settings: &Settings) -> anyhow::Result<Response>
where
    T: Serialize,
{
    let body = serde_json::to_vec(&body)?;
    let mut rng = rand::thread_rng();

    let date = chrono::Utc::now().to_rfc2822();
    let request_url = request.request_url()?;

    let host = request_url.host();
    let path = request_url.path();
    let method = request.method();

    let private_key = RsaPrivateKey::from_pkcs1_pem(&settings.fedi_private_key_pem)?;
    let signing_key = SigningKey::<Sha256>::new(private_key);

    let body_digest = Sha256::digest(&body);
    let digest_header = format!("SHA-256={}", general_purpose::STANDARD.encode(body_digest));

    let signature_string = format!(
        "(request-target): {} {}\nhost: {}\ndate: {}\ndigest: {}",
        method, path, host, date, digest_header
    );
    let signature = signing_key.sign_with_rng(&mut rng, &signature_string.as_bytes());
    let b64_sig = general_purpose::STANDARD.encode(signature.to_bytes());

    let signature_header = format!(
        "keyId=\"{}\",algorithm=\"rsa-sha256\",headers=\"(request-target) host date digest\",signature=\"{}\"",
        settings.activitypub_key_id(),
        b64_sig
    );
    let content_length = body.len().to_string().to_owned();

    let result = request
        .set(header::DATE.as_str(), &date.to_owned())
        .set(header::CONTENT_LENGTH.as_str(), &content_length)
        .set("Signature", &signature_header)
        .set("Digest", &digest_header)
        .send(body.as_slice())?;
    Ok(result)
}

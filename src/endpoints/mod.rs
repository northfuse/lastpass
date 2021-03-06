//! The LastPass API's endpoints.

mod iterations;
mod load_attachment;
mod login;
mod logout;
mod vault;
mod vault_version;

pub use iterations::iterations;
pub use load_attachment::{load_attachment, LoadAttachmentError};
pub use login::{login, LoginError, TwoFactorLoginRequired};
pub use logout::logout;
pub use vault::get_vault;
pub use vault_version::get_vault_version;

use reqwest::{Client, Error, Response};
use serde::Serialize;
use std::fmt::Debug;

/// Typical endpoint errors.
#[derive(Debug, thiserror::Error)]
pub enum EndpointError {
    /// The HTTP client encountered an error.
    #[error("Unable to send the request")]
    HttpClient(#[from] Error),
    /// Unable to parse the XML in the response.
    #[error("Unable to parse the response")]
    XMLParseError(#[from] serde_xml_rs::Error),
    #[error("Unable to parse the response as an integer")]
    BadInteger(
        #[source]
        #[from]
        std::num::ParseIntError,
    ),
    #[error("Unable to base64 decode the payload")]
    Base64(#[from] base64::DecodeError),
}

async fn send<D>(
    client: &Client,
    hostname: &str,
    path: &str,
    data: &D,
) -> Result<Response, Error>
where
    D: Debug + Serialize,
{
    let url = format!("https://{}/{}", hostname, path);

    log::debug!("Sending a request to {}", url);
    log::trace!("Payload: {:#?}", data);
    let response = client
        .post(&url)
        .form(&data)
        .send()
        .await?
        .error_for_status()?;

    log::trace!("Headers: {:#?}", response.headers());

    Ok(response)
}

use crate::Session;
use reqwest::{Client, Error as ReqwestError};
use serde_derive::{Deserialize, Serialize};

pub async fn login(
    client: &Client,
    hostname: &str,
    username: &str,
    hash: &str,
    iterations: usize,
    trusted_id: Option<&str>,
) -> Result<Session, LoginError> {
    let url = format!("https://{}/login.php", hostname);
    let data = LoginData {
        xml: 2,
        username,
        hash,
        iterations,
        includeprivatekeyenc: 1,
        method: "cli",
        outofbandsupported: 1,
        trusted_id,
    };

    log::debug!("Sending a login request to {}", url);
    log::trace!("Payload: {:#?}", data);
    let response = client
        .post(&url)
        .form(&data)
        .send()
        .await?
        .error_for_status()?;

    log::trace!("Headers: {:#?}", response.headers());

    let body = response.text().await?;
    log::trace!("Response: {}", body);

    let doc: LoginResponseDocument = serde_xml_rs::from_str(&body)?;
    log::trace!("Parsed response: {:#?}", doc);

    match doc.response {
        LoginResponse::Error(err) => {
            log::error!("Login failed with {}: {}", err.cause, err.message);

            Err(LoginError::from(err))
        },
        LoginResponse::Ok {
            uid,
            token,
            encoded_private_key,
            session_id,
            username,
            ..
        } => {
            log::info!("Logged in as {}", username);

            Ok(Session {
                uid,
                token,
                encoded_private_key,
                session_id,
            })
        },
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct LoginResponseDocument {
    #[serde(rename = "$value")]
    response: LoginResponse,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum LoginResponse {
    #[serde(rename = "error")]
    Error(ErrorMessage),
    #[serde(rename = "ok")]
    Ok {
        uid: String,
        /// A base64-encoded token.
        token: String,
        #[serde(rename = "privatekeyenc")]
        encoded_private_key: String,
        /// The PHP session ID.
        #[serde(rename = "sessionid")]
        session_id: String,
        /// The user's username.
        #[serde(rename = "lpusername")]
        username: String,
        /// The user's primary email address
        email: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ErrorMessage {
    message: String,
    cause: String,
    enabled_providers: Option<String>,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
struct LoginData<'a> {
    xml: usize,
    username: &'a str,
    hash: &'a str,
    iterations: usize,
    includeprivatekeyenc: usize,
    method: &'a str,
    outofbandsupported: usize,
    #[serde(rename = "uuid")]
    trusted_id: Option<&'a str>,
}

#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("Unable to send the login request")]
    HttpClient(
        #[source]
        #[from]
        ReqwestError,
    ),
    #[error("A new 2FA token is required")]
    TwoFactorLoginRequired(
        #[source]
        #[from]
        TwoFactorLoginRequired,
    ),
    #[error("Unable to parse the login response")]
    ResponseParse(
        #[source]
        #[from]
        serde_xml_rs::Error,
    ),
    /// A catch-all error for when the server rejects a login request and we
    /// can't figure out a more specific error.
    #[error("Login was rejected by the server because {}: {}", cause, message)]
    RejectedByServer { cause: String, message: String },
}

impl From<ErrorMessage> for LoginError {
    fn from(msg: ErrorMessage) -> LoginError {
        if let Some(enabled_providers) = msg.enabled_providers {
            return LoginError::TwoFactorLoginRequired(
                TwoFactorLoginRequired { enabled_providers },
            );
        }

        // we couldn't figure out a better error message
        LoginError::RejectedByServer {
            cause: msg.cause,
            message: msg.message,
        }
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("Re-authenticate with {}", enabled_providers)]
pub struct TwoFactorLoginRequired {
    enabled_providers: String,
}

impl TwoFactorLoginRequired {
    pub fn providers(&self) -> impl Iterator<Item = &'_ str> + '_ {
        self.enabled_providers.split_ascii_whitespace()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_login_error_2fa_missing() {
        let src = include_str!("login_response_googleauthrequired.xml");
        let should_be = LoginResponseDocument {
            response: LoginResponse::Error(ErrorMessage {
                message: String::from("Google Authenticator authentication required! Update your browser extension so you can enter it."),
                cause: String::from("googleauthrequired"),
                enabled_providers: Some(String::from("googleauth")),
            }),
        };

        let got: LoginResponseDocument = serde_xml_rs::from_str(src).unwrap();

        assert_eq!(got, should_be);
    }

    #[test]
    fn parse_happy_login_response() {
        let src = include_str!("login_response_okay.xml");
        let should_be = LoginResponseDocument {
            response: LoginResponse::Ok {
                email: String::from("michaelfbryan@gmail.com"),
                username: String::from("michaelfbryan@gmail.com"),
                uid: String::from("999999999"),
                session_id: String::from("SESSIONID1234"),
                encoded_private_key: String::from("SUPERSECRETPRIVATEKEY"),
                token: String::from("BASE64ENCODEDTOKEN="),
            },
        };

        let got: LoginResponseDocument = serde_xml_rs::from_str(src).unwrap();

        assert_eq!(got, should_be);
    }
}

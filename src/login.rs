use std::str::FromStr;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::time::Duration;

const LOGIN_URL: &'static str = "https://www.toontownrewritten.com/api/login?format=json";
const SERVICE_NAME: &'static str = "ttr-launcher-oxide";

pub fn reset_keyring(username: &str) {
    let keyring = keyring::Keyring::new(SERVICE_NAME, username);
    if let Err(err) = keyring.delete_password() {
        eprintln!("Failed to delete password!\n{}", err);
    }
}

pub async fn login(
    username: String,
    password: Option<String>,
    save_password: bool,
) -> Option<LoginToken> {
    let password = match password {
        Some(p) => {
            if save_password {
                let keyring = keyring::Keyring::new(SERVICE_NAME, username.as_str());
                if let Err(err) = keyring.set_password(p.as_str()) {
                    eprintln!("Failed to save password in keyring.\n{}", err);
                }
            }
            p.to_string()
        }
        None => {
            let keyring = keyring::Keyring::new(SERVICE_NAME, username.as_str());
            match keyring.get_password() {
                Ok(pass) => pass,
                Err(err) => {
                    eprintln!("Failed to retrieve stored password from keyring.\n{}", err);
                    return None;
                }
            }
        }
    };
    let credentials = Credentials {
        username: username.to_string(),
        password,
    };
    let client = reqwest::Client::new();
    let initial_request = client
        .post(LOGIN_URL)
        .form(&credentials)
        .build()
        .expect("Forming request");

    match client.execute(initial_request).await {
        Ok(res) => {
            let response_result = res.json::<LoginResponse>().await;
            handle_login_response(&client, response_result).await
        }
        Err(err) => {
            eprintln!("An error occurred while executing the request.\n{}", err);
            None
        }
    }
}

macro_rules! response_result_none_return {
    ($response_result:expr) => {{
        if let Err(err) = $response_result {
            eprintln!("Login error\n{}", err);
            return None;
        }
        $response_result.unwrap()
    }};
}

async fn handle_login_response(
    client: &Client,
    response_result: reqwest::Result<LoginResponse>,
) -> Option<LoginToken> {
    let response = response_result_none_return!(response_result);
    dispatch_2fa_possible(client, response).await
}

async fn dispatch_no_2fa(client: &Client, response: LoginResponse) -> Option<LoginToken> {
    match response.success {
        LoginResult::Success => Some(LoginToken {
            server: response.gameserver.unwrap(),
            cookie: response.cookie.unwrap(),
        }),
        LoginResult::Partial => panic!("2FA asserted not possible, but came back partial!"),
        LoginResult::Failure => None,
        LoginResult::Delayed => {
            let eta = u32::from_str(response.eta.as_ref().unwrap()).expect("parsing eta number");
            let position = u32::from_str(response.position.as_ref().unwrap())
                .expect("parsing position number");
            let res = queue(&client, response.queue_token.unwrap(), eta, position).await;
            if res.is_none() {
                eprintln!("An error occurred while moving through the queue.");
                return None;
            }
            res
        }
    }
}

async fn dispatch_2fa_possible(client: &Client, response: LoginResponse) -> Option<LoginToken> {
    if response.success.is_partial() {
        two_factor(&client, response.response_token.unwrap()).await
    } else {
        dispatch_no_2fa(client, response).await
    }
}

async fn two_factor(client: &Client, token: String) -> Option<LoginToken> {
    let mut token = token;
    loop {
        let totp = rprompt::prompt_reply_stdout("2 factor TOTP code: ").expect("Reading 2fa stdin");
        let totp_request = client
            .post(LOGIN_URL)
            .form(&TOTPRequest { totp, token })
            .build()
            .expect("Forming request 2fa");
        match client.execute(totp_request).await {
            Ok(res) => {
                let response_result = res.json::<LoginResponse>().await;
                let response = response_result_none_return!(response_result);
                if response.success.is_partial() {
                    println!("Unrecognized TOTP code.");
                    token = response.response_token.unwrap();
                } else {
                    return dispatch_no_2fa(client, response).await;
                }
            }
            Err(err) => {
                eprintln!(
                    "An error occurred while executing the 2fa request.\n{}",
                    err
                );
                return None;
            }
        }
    }
}

async fn queue(client: &Client, token: String, eta: u32, position: u32) -> Option<LoginToken> {
    let mut eta = eta;
    let mut position = position;
    let mut token = token;
    loop {
        println!("In queue -- Position: {}, ETA: {} seconds.", position, eta);
        async_std::task::sleep(Duration::from_secs(eta as u64)).await;
        let queue_request = client
            .post(LOGIN_URL)
            .form(&QueueToken {
                queue_token: token.clone(),
            })
            .build()
            .expect("Forming request queue");

        let resp = client.execute(queue_request).await;
        let resp = if resp.is_ok() {
            resp.unwrap().json::<LoginResponse>().await
        } else {
            Err(resp.unwrap_err())
        };
        match resp {
            Err(err) => {
                eprintln!("Failed to update position in queue.\n{}", err);
                return None;
            }
            Ok(resp) => {
                if resp.success.is_success() {
                    return Some(LoginToken {
                        server: resp.gameserver.unwrap(),
                        cookie: resp.cookie.unwrap(),
                    });
                } else {
                    eta = u32::from_str(resp.eta.as_ref().unwrap()).expect("parsing eta number");
                    position = u32::from_str(resp.position.as_ref().unwrap())
                        .expect("parsing position number");
                    token = resp.queue_token.as_ref().unwrap().clone();
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct LoginToken {
    pub server: String,
    pub cookie: String,
}

#[derive(Serialize)]
struct Credentials {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
struct QueueToken {
    #[serde(rename = "queueToken")]
    queue_token: String,
}

#[derive(Serialize)]
struct TOTPRequest {
    #[serde(rename = "appToken")]
    totp: String,
    #[serde(rename = "authToken")]
    token: String,
}

#[derive(Deserialize)]
struct LoginResponse {
    pub success: LoginResult,
    pub banner: Option<String>,
    #[serde(rename = "responseToken")]
    pub response_token: Option<String>,
    pub gameserver: Option<String>,
    pub cookie: Option<String>,
    pub eta: Option<String>,
    pub position: Option<String>,
    #[serde(rename = "queueToken")]
    pub queue_token: Option<String>,
}

#[derive(Deserialize)]
enum LoginResult {
    #[serde(rename = "true")]
    Success,
    #[serde(rename = "delayed")]
    Delayed,
    #[serde(rename = "partial")]
    Partial,
    #[serde(rename = "false")]
    Failure,
}

#[allow(unused)]
impl LoginResult {
    pub fn is_success(&self) -> bool {
        if let Self::Success = self {
            true
        } else {
            false
        }
    }
    pub fn is_delayed(&self) -> bool {
        if let Self::Delayed = self {
            true
        } else {
            false
        }
    }
    pub fn is_partial(&self) -> bool {
        if let Self::Partial = self {
            true
        } else {
            false
        }
    }
    pub fn is_failure(&self) -> bool {
        if let Self::Failure = self {
            true
        } else {
            false
        }
    }
}

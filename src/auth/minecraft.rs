use super::models::{AuthError, EntitlementsResponse, MinecraftLogin, MinecraftProfile};

const MC_LOGIN_URL: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const MC_ENTITLEMENTS_URL: &str = "https://api.minecraftservices.com/entitlements/mcstore";
const MC_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
const SESSION_JOIN_URL: &str = "https://sessionserver.mojang.com/session/minecraft/join";

pub fn join_server(access_token: &str, uuid: &str, server_hash: &str) -> Result<(), AuthError> {
    #[derive(serde::Serialize)]
    struct JoinRequest<'a> {
        #[serde(rename = "accessToken")]
        access_token: &'a str,
        #[serde(rename = "selectedProfile")]
        selected_profile: String,
        #[serde(rename = "serverId")]
        server_id: &'a str,
    }
    let response = reqwest::blocking::Client::new()
        .post(SESSION_JOIN_URL)
        .json(&JoinRequest {
            access_token,
            selected_profile: uuid.replace('-', ""),
            server_id: server_hash,
        })
        .send()?;
    if response.status().as_u16() == 204 {
        Ok(())
    } else {
        let status = response.status();
        Err(AuthError::OAuth(format!(
            "Session join failed ({status}): {}",
            response.text().unwrap_or_default()
        )))
    }
}

/// Login to Minecraft Services using XSTS token.
///
/// POST https://api.minecraftservices.com/authentication/login_with_xbox
/// Body:
/// ```json
/// {
///   "identityToken": "XBL3.0 x=<uhs>;<xsts_token>"
/// }
/// ```
pub fn login_with_xbox(uhs: &str, xsts_token: &str) -> Result<MinecraftLogin, AuthError> {
    let client = reqwest::blocking::Client::new();

    #[derive(serde::Serialize)]
    struct LoginRequest {
        #[serde(rename = "identityToken")]
        identity_token: String,
    }

    let identity_token = format!("XBL3.0 x={};{}", uhs, xsts_token);
    let body = LoginRequest { identity_token };

    let resp = client.post(MC_LOGIN_URL).json(&body).send()?;
    let status = resp.status();
    let text = resp.text()?;

    if !status.is_success() {
        return Err(AuthError::Xbl(format!(
            "Minecraft login failed ({}): {}",
            status, text
        )));
    }

    #[derive(serde::Deserialize)]
    struct LoginResponse {
        access_token: String,
        expires_in: u64,
    }

    let result: LoginResponse = serde_json::from_str(&text)?;

    Ok(MinecraftLogin {
        access_token: result.access_token,
        expires_in: result.expires_in,
    })
}

/// Check if the user owns Minecraft Java Edition.
///
/// GET https://api.minecraftservices.com/entitlements/mcstore
pub fn check_entitlements(mc_access_token: &str) -> Result<bool, AuthError> {
    let client = reqwest::blocking::Client::new();

    let resp = client
        .get(MC_ENTITLEMENTS_URL)
        .bearer_auth(mc_access_token)
        .send()?;

    let status = resp.status();
    if !status.is_success() {
        return Err(AuthError::Http(format!(
            "Entitlements check failed ({}): {}",
            status,
            resp.text().unwrap_or_default()
        )));
    }

    let entitlements: EntitlementsResponse = resp.json()?;

    let owns_minecraft = entitlements.items.iter().any(|item| {
        item.name.as_deref() == Some("product_minecraft")
            || item.name.as_deref() == Some("game_minecraft")
    });

    Ok(owns_minecraft)
}

/// Get the Minecraft player profile.
///
/// GET https://api.minecraftservices.com/minecraft/profile
pub fn get_profile(mc_access_token: &str) -> Result<MinecraftProfile, AuthError> {
    let client = reqwest::blocking::Client::new();

    let resp = client
        .get(MC_PROFILE_URL)
        .bearer_auth(mc_access_token)
        .send()?;

    let status = resp.status();
    let text = resp.text()?;

    if status.as_u16() == 404 {
        return Err(AuthError::NoProfile);
    }
    if !status.is_success() {
        return Err(AuthError::Http(format!(
            "Profile request failed ({}): {}",
            status, text
        )));
    }

    let profile: MinecraftProfile = serde_json::from_str(&text)?;
    Ok(profile)
}

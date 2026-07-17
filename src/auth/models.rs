use serde::{Deserialize, Serialize};

/// Unified authentication state for a single account.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Account {
    /// Microsoft OAuth2 refresh token (long-lived).
    pub microsoft_refresh_token: Option<String>,
    /// Minecraft access token (short-lived, ~24h).
    pub minecraft_access_token: Option<String>,
    /// Unix epoch (seconds) when minecraft_access_token expires.
    pub minecraft_token_expiry: Option<i64>,
    /// Minecraft player UUID (with dashes).
    pub uuid: Option<String>,
    /// Minecraft player name.
    pub username: Option<String>,
    /// Skin JSON from the profile endpoint.
    pub skins: Option<Vec<Skin>>,
    /// Cape JSON from the profile endpoint.
    pub capes: Option<Vec<Cape>>,
}

impl Account {
    pub fn empty() -> Self {
        Self {
            microsoft_refresh_token: None,
            minecraft_access_token: None,
            minecraft_token_expiry: None,
            uuid: None,
            username: None,
            skins: None,
            capes: None,
        }
    }

    pub fn is_online(&self) -> bool {
        self.minecraft_access_token.is_some() && self.username.is_some()
    }

    pub fn token_valid(&self) -> bool {
        match self.minecraft_token_expiry {
            Some(exp) => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                now < exp - 60
            }
            None => false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Skin {
    pub id: String,
    pub state: String,
    pub url: String,
    pub variant: String,
    pub texture_key: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Cape {
    pub id: String,
    pub state: String,
    pub url: String,
    pub alias: Option<String>,
}

/// Transient OAuth2 tokens from Microsoft.
pub struct MicrosoftToken {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

/// XBL 2.0 authentication result.
pub struct XblToken {
    pub token: String,
    pub uhs: String,
}

/// XSTS authorization result.
pub struct XstsToken {
    pub token: String,
    pub uhs: String,
}

/// Minecraft Services login result.
pub struct MinecraftLogin {
    pub access_token: String,
    pub expires_in: u64,
}

/// Minecraft profile (from /minecraft/profile).
#[derive(Clone, Debug, Deserialize)]
pub struct MinecraftProfile {
    pub id: String,
    pub name: String,
    pub skins: Option<Vec<Skin>>,
    pub capes: Option<Vec<Cape>>,
}

/// MC Store entitlements response.
#[derive(Clone, Debug, Deserialize)]
pub struct EntitlementsResponse {
    pub items: Vec<EntitlementItem>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EntitlementItem {
    pub name: Option<String>,
    pub signature: Option<String>,
    #[serde(rename = "signatureV2")]
    pub signature_v2: Option<String>,
}

/// Launch arguments generated from a valid account.
#[derive(Clone, Debug)]
pub struct LaunchArgs {
    pub username: String,
    pub uuid: String,
    pub access_token: String,
    pub user_type: String,
}

impl LaunchArgs {
    pub fn to_vec(&self) -> Vec<String> {
        vec![
            "--username".into(),
            self.username.clone(),
            "--uuid".into(),
            self.uuid.clone(),
            "--accessToken".into(),
            self.access_token.clone(),
            "--userType".into(),
            self.user_type.clone(),
        ]
    }
}

/// Errors that can occur during authentication.
#[derive(Debug)]
pub enum AuthError {
    Http(String),
    Json(String),
    OAuth(String),
    Xbl(String),
    Xsts { code: i64, message: String },
    NotOwned,
    NoProfile,
    Network(String),
    Io(String),
    Cache(String),
    Cancelled,
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::Http(msg) => write!(f, "HTTP error: {}", msg),
            AuthError::Json(msg) => write!(f, "JSON error: {}", msg),
            AuthError::OAuth(msg) => write!(f, "OAuth error: {}", msg),
            AuthError::Xbl(msg) => write!(f, "XBL error: {}", msg),
            AuthError::Xsts { code, message } => {
                write!(f, "XSTS error {}: {}", code, message)
            }
            AuthError::NotOwned => write!(f, "User does not own Minecraft Java Edition"),
            AuthError::NoProfile => write!(f, "No Minecraft profile found"),
            AuthError::Network(msg) => write!(f, "Network error: {}", msg),
            AuthError::Io(msg) => write!(f, "IO error: {}", msg),
            AuthError::Cache(msg) => write!(f, "Cache error: {}", msg),
            AuthError::Cancelled => write!(f, "Authentication cancelled"),
        }
    }
}

impl From<reqwest::Error> for AuthError {
    fn from(e: reqwest::Error) -> Self {
        AuthError::Http(e.to_string())
    }
}

impl From<serde_json::Error> for AuthError {
    fn from(e: serde_json::Error) -> Self {
        AuthError::Json(e.to_string())
    }
}

impl From<std::io::Error> for AuthError {
    fn from(e: std::io::Error) -> Self {
        AuthError::Io(e.to_string())
    }
}

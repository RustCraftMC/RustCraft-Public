use super::cache;
use super::minecraft;
use super::models::{Account, AuthError, LaunchArgs};
use super::oauth;
use super::xbox;

const MICROSOFT_CLIENT_ID: &str = "54fd49e4-2103-4044-9603-2b028c814ec3";
const OAUTH_REDIRECT_PORT: u16 = 9812;

/// Top-level authentication service.
///
/// Handles:
/// - Full Microsoft OAuth2 + XBL + XSTS + Minecraft Services login flow
/// - Silent token refresh from cache
/// - Launch argument generation
pub struct AuthService;

impl AuthService {
    /// Attempt to log in using cached tokens.
    ///
    /// Strategy:
    /// 1. Load cached account.
    /// 2. If Minecraft token is still valid, return immediately.
    /// 3. If expired, try to refresh via Microsoft refresh token.
    /// 4. If refresh fails, return None — caller should run `full_login()`.
    pub fn try_auto_login() -> Result<Option<Account>, AuthError> {
        let mut account = match cache::load_account()? {
            Some(a) => a,
            None => return Ok(None),
        };

        if account.token_valid() {
            return Ok(Some(account));
        }

        if let Some(ref refresh_token) = account.microsoft_refresh_token.clone() {
            match Self::refresh_minecraft_token(refresh_token) {
                Ok(updated) => {
                    cache::save_account(&updated)?;
                    return Ok(Some(updated));
                }
                Err(_) => {
                    // Refresh token expired or invalid — need full re-login
                    if let Some(uuid) = account.uuid.as_deref() {
                        cache::remove_account(uuid)?;
                    }
                    return Ok(None);
                }
            }
        }

        Ok(None)
    }

    /// Perform the full login flow:
    ///   1. Microsoft OAuth2 (PKCE) → browser login
    ///   2. XBL 2.0 authentication
    ///   3. XSTS authorization
    ///   4. Minecraft Services login
    ///   5. Entitlements check
    ///   6. Profile retrieval
    ///   7. Cache the result
    pub fn full_login() -> Result<Account, AuthError> {
        // Step 1: Microsoft OAuth2
        let ms_token = oauth::microsoft_login(MICROSOFT_CLIENT_ID, OAUTH_REDIRECT_PORT)?;

        // Step 2: XBL authentication
        let xbl = xbox::xbl_authenticate(&ms_token.access_token)?;

        // Step 3: XSTS authorization
        let xsts = xbox::xsts_authorize(&xbl.token)?;

        // Step 4: Minecraft Services login
        let mc_login = minecraft::login_with_xbox(&xsts.uhs, &xsts.token)?;

        // Step 5: Check game ownership
        let owns = minecraft::check_entitlements(&mc_login.access_token)?;
        if !owns {
            return Err(AuthError::NotOwned);
        }

        // Step 6: Get player profile
        let profile = minecraft::get_profile(&mc_login.access_token)?;

        let expiry = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
            + mc_login.expires_in as i64;

        let mut account = Account {
            microsoft_refresh_token: Some(ms_token.refresh_token),
            minecraft_access_token: Some(mc_login.access_token),
            minecraft_token_expiry: Some(expiry),
            uuid: Some(Self::format_uuid(&profile.id)),
            username: Some(profile.name.clone()),
            skins: profile.skins.clone(),
            capes: profile.capes.clone(),
        };

        cache::save_account(&account)?;
        Ok(account)
    }

    /// Refresh the Minecraft token using a Microsoft refresh token.
    /// Runs the full XBL → XSTS → Minecraft pipeline with a new Microsoft token.
    fn refresh_minecraft_token(microsoft_refresh_token: &str) -> Result<Account, AuthError> {
        let ms_token = oauth::microsoft_refresh(MICROSOFT_CLIENT_ID, microsoft_refresh_token)?;

        let xbl = xbox::xbl_authenticate(&ms_token.access_token)?;
        let xsts = xbox::xsts_authorize(&xbl.token)?;
        let mc_login = minecraft::login_with_xbox(&xsts.uhs, &xsts.token)?;
        let profile = minecraft::get_profile(&mc_login.access_token)?;

        let expiry = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
            + mc_login.expires_in as i64;

        Ok(Account {
            microsoft_refresh_token: Some(ms_token.refresh_token),
            minecraft_access_token: Some(mc_login.access_token),
            minecraft_token_expiry: Some(expiry),
            uuid: Some(Self::format_uuid(&profile.id)),
            username: Some(profile.name.clone()),
            skins: profile.skins,
            capes: profile.capes,
        })
    }

    /// Return a usable cached account, refreshing its Minecraft access token when
    /// necessary.  Connection code calls this before an online-mode handshake.
    pub fn active_account() -> Result<Account, AuthError> {
        Self::try_auto_login()?
            .ok_or_else(|| AuthError::OAuth("No authenticated Microsoft account selected".into()))
    }

    /// Generate launch arguments from a valid account.
    pub fn generate_launch_args(account: &Account) -> Result<LaunchArgs, AuthError> {
        let username = account
            .username
            .clone()
            .ok_or_else(|| AuthError::OAuth("No username in cached account".into()))?;
        let uuid = account
            .uuid
            .clone()
            .ok_or_else(|| AuthError::OAuth("No UUID in cached account".into()))?;
        let access_token = account
            .minecraft_access_token
            .clone()
            .ok_or_else(|| AuthError::OAuth("No access token in cached account".into()))?;

        Ok(LaunchArgs {
            username,
            uuid,
            access_token,
            user_type: "msa".into(),
        })
    }

    /// Format a UUID string with dashes (8-4-4-4-12).
    fn format_uuid(raw: &str) -> String {
        let clean = raw.replace('-', "");
        if clean.len() == 32 {
            format!(
                "{}-{}-{}-{}-{}",
                &clean[0..8],
                &clean[8..12],
                &clean[12..16],
                &clean[16..20],
                &clean[20..32]
            )
        } else {
            raw.to_string()
        }
    }

    /// Generate an MC-style offline player UUID from username.
    /// Uses the "OfflinePlayer:{name}" hash (matching vanilla offline mode).
    pub fn offline_uuid(name: &str) -> String {
        use std::hash::{Hash, Hasher};
        let input = format!("OfflinePlayer:{name}");
        let mut h = std::collections::hash_map::DefaultHasher::new();
        input.hash(&mut h);
        let hash = h.finish();
        // Construct a UUID v3-style from the hash
        format!(
            "{:08x}-{:04x}-3{:03x}-a{:03x}-{:012x}",
            hash as u32,
            (hash >> 32) as u16,
            (hash >> 48) as u16 & 0xfff,
            (hash >> 12) as u16 & 0xfff,
            hash & 0xffff_ffff_ffff
        )
    }
}

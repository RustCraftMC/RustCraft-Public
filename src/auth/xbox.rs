use super::models::{AuthError, XblToken, XstsToken};

const XBL_AUTH_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_AUTH_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";

/// Authenticate with Xbox Live (XBL 2.0) using a Microsoft access token.
///
/// POST https://user.auth.xboxlive.com/user/authenticate
/// Headers: x-xbl-contract-version: 1
/// Body:
/// ```json
/// {
///   "Properties": {
///     "AuthMethod": "RPS",
///     "SiteName": "user.auth.xboxlive.com",
///     "RpsTicket": "d=<access_token>"
///   },
///   "RelyingParty": "http://auth.xboxlive.com",
///   "TokenType": "JWT"
/// }
/// ```
pub fn xbl_authenticate(ms_access_token: &str) -> Result<XblToken, AuthError> {
    let client = reqwest::blocking::Client::new();

    #[derive(serde::Serialize)]
    struct XblProperties {
        #[serde(rename = "AuthMethod")]
        auth_method: String,
        #[serde(rename = "SiteName")]
        site_name: String,
        #[serde(rename = "RpsTicket")]
        rps_ticket: String,
    }

    #[derive(serde::Serialize)]
    struct XblRequest {
        #[serde(rename = "Properties")]
        properties: XblProperties,
        #[serde(rename = "RelyingParty")]
        relying_party: String,
        #[serde(rename = "TokenType")]
        token_type: String,
    }

    let body = XblRequest {
        properties: XblProperties {
            auth_method: "RPS".into(),
            site_name: "user.auth.xboxlive.com".into(),
            rps_ticket: format!("d={}", ms_access_token),
        },
        relying_party: "http://auth.xboxlive.com".into(),
        token_type: "JWT".into(),
    };

    let resp = client
        .post(XBL_AUTH_URL)
        .header("x-xbl-contract-version", "1")
        .json(&body)
        .send()?;

    let status = resp.status();
    let text = resp.text()?;

    if !status.is_success() {
        return Err(AuthError::Xbl(format!(
            "XBL auth failed ({}): {}",
            status, text
        )));
    }

    #[derive(serde::Deserialize)]
    struct XblResponse {
        #[serde(rename = "Token")]
        token: String,
        #[serde(rename = "DisplayClaims")]
        display_claims: DisplayClaims,
    }

    #[derive(serde::Deserialize)]
    struct DisplayClaims {
        xui: Vec<XuiClaim>,
    }

    #[derive(serde::Deserialize)]
    struct XuiClaim {
        uhs: String,
    }

    let result: XblResponse = serde_json::from_str(&text)?;
    let uhs = result
        .display_claims
        .xui
        .first()
        .map(|x| x.uhs.clone())
        .ok_or_else(|| AuthError::Xbl("No uhs in XBL response".into()))?;

    Ok(XblToken {
        token: result.token,
        uhs,
    })
}

/// Authorize with XSTS using an XBL token.
///
/// POST https://xsts.auth.xboxlive.com/xsts/authorize
/// Headers: x-xbl-contract-version: 1
/// Body:
/// ```json
/// {
///   "Properties": {
///     "SandboxId": "RETAIL",
///     "UserTokens": ["<xbl_token>"]
///   },
///   "RelyingParty": "rp://api.minecraftservices.com/",
///   "TokenType": "JWT"
/// }
/// ```
pub fn xsts_authorize(xbl_token: &str) -> Result<XstsToken, AuthError> {
    let client = reqwest::blocking::Client::new();

    #[derive(serde::Serialize)]
    struct XstsProperties {
        #[serde(rename = "SandboxId")]
        sandbox_id: String,
        #[serde(rename = "UserTokens")]
        user_tokens: Vec<String>,
    }

    #[derive(serde::Serialize)]
    struct XstsRequest {
        #[serde(rename = "Properties")]
        properties: XstsProperties,
        #[serde(rename = "RelyingParty")]
        relying_party: String,
        #[serde(rename = "TokenType")]
        token_type: String,
    }

    let body = XstsRequest {
        properties: XstsProperties {
            sandbox_id: "RETAIL".into(),
            user_tokens: vec![xbl_token.to_string()],
        },
        relying_party: "rp://api.minecraftservices.com/".into(),
        token_type: "JWT".into(),
    };

    let resp = client
        .post(XSTS_AUTH_URL)
        .header("x-xbl-contract-version", "1")
        .json(&body)
        .send()?;

    let status = resp.status();
    let text = resp.text()?;

    match status.as_u16() {
        200 => {
            #[derive(serde::Deserialize)]
            struct XstsResponse {
                #[serde(rename = "Token")]
                token: String,
                #[serde(rename = "DisplayClaims")]
                display_claims: DisplayClaims,
            }

            #[derive(serde::Deserialize)]
            struct DisplayClaims {
                xui: Vec<XuiClaim>,
            }

            #[derive(serde::Deserialize)]
            struct XuiClaim {
                uhs: String,
            }

            let result: XstsResponse = serde_json::from_str(&text)?;
            let uhs = result
                .display_claims
                .xui
                .first()
                .map(|x| x.uhs.clone())
                .ok_or_else(|| AuthError::Xbl("No uhs in XSTS response".into()))?;

            Ok(XstsToken {
                token: result.token,
                uhs,
            })
        }
        401 => {
            // XSTS error — parse XErr for known issues
            let xerr_code = extract_xerr(&text);
            let message = match xerr_code {
                Some(2148916233) => {
                    "This account does not have an Xbox Live profile. \
                     Please create one at https://account.xbox.com/profile"
                        .into()
                }
                Some(2148916238) => {
                    "The account is a child account and requires parental consent. \
                     Use a Microsoft Family account or wait until the account age requirement is met."
                        .into()
                }
                Some(2148916235) => {
                    "Xbox Live is not available in your region. \
                     Check https://www.xbox.com/regions"
                        .into()
                }
                _ => format!("Unknown XSTS error (status {})", status),
            };
            Err(AuthError::Xsts {
                code: xerr_code.unwrap_or(status.as_u16() as i64),
                message,
            })
        }
        _ => Err(AuthError::Xbl(format!(
            "XSTS auth failed ({}): {}",
            status, text
        ))),
    }
}

fn extract_xerr(body: &str) -> Option<i64> {
    #[derive(serde::Deserialize)]
    struct XErrResponse {
        #[serde(rename = "XErr")]
        xerr: Option<i64>,
    }
    if let Ok(r) = serde_json::from_str::<XErrResponse>(body) {
        return r.xerr;
    }
    None
}

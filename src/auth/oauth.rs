use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use rand::Rng;
use sha2::{Digest, Sha256};

use super::models::{AuthError, MicrosoftToken};

const MICROSOFT_AUTHORIZE_URL: &str = "https://login.live.com/oauth20_authorize.srf";
const MICROSOFT_TOKEN_URL: &str = "https://login.live.com/oauth20_token.srf";
const SCOPES: &str = "XboxLive.signin XboxLive.offline_access";
const REDIRECT_URI: &str = "http://localhost:9812/in_game_account_switcher_long_enough_uri_to_prevent_accidental_leaks_on_screensharing_even_if_you_have_like_extremely_big_screen_though_it_might_not_mork_but_we_will_try_it_anyway_to_prevent_funny_things_from_happening_or_something";

fn base64url_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

fn generate_code_verifier() -> String {
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
    base64url_encode(&bytes)
}

fn code_challenge(verifier: &str) -> String {
    let hash = Sha256::digest(verifier.as_bytes());
    base64url_encode(&hash)
}

/// Perform the full Microsoft OAuth2 authorization code + PKCE flow.
///
/// 1. Starts a local HTTP server on `127.0.0.1:<redirect_port>`.
/// 2. Opens the browser to the Microsoft login page.
/// 3. Receives the authorization code via the redirect callback.
/// 4. Exchanges the code for access + refresh tokens.
pub fn microsoft_login(client_id: &str, redirect_port: u16) -> Result<MicrosoftToken, AuthError> {
    let verifier = generate_code_verifier();
    let challenge = code_challenge(&verifier);

    let auth_url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&display=touch&scope={}&code_challenge={}&code_challenge_method=S256&prompt=select_account",
        MICROSOFT_AUTHORIZE_URL,
        urlencode(client_id),
        urlencode(REDIRECT_URI),
        urlencode(SCOPES),
        challenge,
    );

    let listener = TcpListener::bind("127.0.0.1:9812")
        .map_err(|e| AuthError::OAuth(format!("Failed to bind port 9812: {}", e)))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| AuthError::OAuth(format!("Failed to set non-blocking: {}", e)))?;

    let auth_code: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let cancelled = Arc::new(AtomicBool::new(false));

    open_browser(&auth_url)
        .map_err(|e| AuthError::OAuth(format!("Failed to open browser: {}", e)))?;

    // Accept connections with timeout polling
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);

    while deadline.elapsed().as_secs() < 120 {
        match listener.accept() {
            Ok((stream, _)) => {
                let code_clone = auth_code.clone();
                let cancelled_clone = cancelled.clone();
                handle_redirect(stream, code_clone, cancelled_clone);
                if cancelled.load(Ordering::SeqCst) {
                    return Err(AuthError::Cancelled);
                }
                let has_code = { auth_code.lock().unwrap().is_some() };
                if has_code {
                    break;
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                return Err(AuthError::OAuth(format!("Server error: {}", e)));
            }
        }
    }

    let code = auth_code
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| AuthError::OAuth("No authorization code received (timeout)".into()))?;

    exchange_code_for_token(client_id, &code, &verifier)
}

fn handle_redirect(
    mut stream: TcpStream,
    auth_code: Arc<Mutex<Option<String>>>,
    cancelled: Arc<AtomicBool>,
) {
    let mut buf = [0u8; 4096];
    let n = match stream.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return,
    };
    let request = String::from_utf8_lossy(&buf[..n]);

    let (status_line, body) = if let Some(code) = extract_code_from_request(&request) {
        *auth_code.lock().unwrap() = Some(code);
        (
                "HTTP/1.1 200 OK",
                "<html><body><p>Authentication successful! You may close this window.</p></body></html>",
            )
    } else if request.contains("error=access_denied") || request.contains("error=user_cancelled") {
        cancelled.store(true, Ordering::SeqCst);
        (
            "HTTP/1.1 200 OK",
            "<html><body><p>Authentication cancelled. You may close this window.</p></body></html>",
        )
    } else {
        (
                "HTTP/1.1 400 Bad Request",
                "<html><body><p>Authentication failed. No authorization code received.</p></body></html>",
            )
    };

    let response = format!(
        "{}\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
        status_line,
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
}

fn extract_code_from_request(request: &str) -> Option<String> {
    let first_line = request.lines().next()?;
    let path = first_line.split_whitespace().nth(1)?;

    let query = if let Some(pos) = path.find('?') {
        &path[pos + 1..]
    } else {
        return None;
    };

    for pair in query.split('&') {
        if let Some(value) = pair.strip_prefix("code=") {
            return Some(url_decode(value));
        }
    }
    None
}

fn exchange_code_for_token(
    client_id: &str,
    code: &str,
    verifier: &str,
) -> Result<MicrosoftToken, AuthError> {
    let client = reqwest::blocking::Client::new();
    let params = [
        ("client_id", client_id),
        ("redirect_uri", REDIRECT_URI),
        ("code", code),
        ("code_verifier", verifier),
        ("grant_type", "authorization_code"),
    ];

    let resp = client.post(MICROSOFT_TOKEN_URL).form(&params).send()?;

    let status = resp.status();
    let text = resp.text()?;

    if !status.is_success() {
        return Err(AuthError::OAuth(format!(
            "Token exchange failed ({}): {}",
            status, text
        )));
    }

    #[derive(serde::Deserialize)]
    struct TokenResponse {
        access_token: String,
        refresh_token: Option<String>,
        expires_in: u64,
    }

    let token: TokenResponse = serde_json::from_str(&text)?;
    let refresh_token = token
        .refresh_token
        .ok_or_else(|| AuthError::OAuth("No refresh_token in response".into()))?;

    Ok(MicrosoftToken {
        access_token: token.access_token,
        refresh_token,
        expires_in: token.expires_in,
    })
}

/// Refresh a Microsoft access token using a refresh token.
pub fn microsoft_refresh(
    client_id: &str,
    refresh_token: &str,
) -> Result<MicrosoftToken, AuthError> {
    let client = reqwest::blocking::Client::new();
    let params = [
        ("client_id", client_id),
        ("refresh_token", refresh_token),
        ("grant_type", "refresh_token"),
    ];

    let resp = client.post(MICROSOFT_TOKEN_URL).form(&params).send()?;
    let status = resp.status();
    let text = resp.text()?;

    if !status.is_success() {
        return Err(AuthError::OAuth(format!(
            "Token refresh failed ({}): {}",
            status, text
        )));
    }

    #[derive(serde::Deserialize)]
    struct RefreshResponse {
        access_token: String,
        refresh_token: Option<String>,
        expires_in: u64,
    }

    let token: RefreshResponse = serde_json::from_str(&text)?;

    Ok(MicrosoftToken {
        access_token: token.access_token,
        refresh_token: token
            .refresh_token
            .unwrap_or_else(|| refresh_token.to_string()),
        expires_in: token.expires_in,
    })
}

fn open_browser(url: &str) -> Result<(), std::io::Error> {
    if cfg!(target_os = "windows") {
        std::process::Command::new("cmd")
            .args(["/c", "start", &url.replace('&', "^&")])
            .spawn()?;
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn()?;
    } else {
        std::process::Command::new("xdg-open").arg(url).spawn()?;
    }
    Ok(())
}

/// Percent-encode a string for URL query parameters.
fn urlencode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            b' ' => result.push_str("%20"),
            _ => result.push_str(&format!("%{:02X}", byte)),
        }
    }
    result
}

/// Percent-decode a string.
fn url_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '+' {
            result.push(' ');
        } else if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            }
        } else {
            result.push(c);
        }
    }
    result
}

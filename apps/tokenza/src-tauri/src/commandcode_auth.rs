//! Browser sign-in for Command Code, mirroring the official CLI's loopback flow.
//!
//! The Studio page posts the key to a one-shot server on 127.0.0.1; Tokenza validates it and
//! keeps it in the OS credential store. The CLI's `auth.json` is neither read nor written here.

use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;
use std::time::{Duration, Instant};

use serde_json::Value;
use tauri::{AppHandle, State};

use crate::state::AppState;
use crate::{scheduler, secrets};

const AUTH_URL: &str = "https://commandcode.ai/studio/auth/cli";
const COMPLETE_PATH: &str = "/callback/complete";
const CALLBACK_PATH: &str = "/callback";
/// Same window the CLI gives the browser; long enough for a sign-in and a 2FA prompt.
const TIMEOUT: Duration = Duration::from_secs(300);
/// The browser needs a moment to follow the redirect to the "done" page before the server closes.
const LANDING_GRACE: Duration = Duration::from_secs(10);
const MAX_BODY: usize = 16 * 1024;
const MAX_HEAD: usize = 16 * 1024;

fn page(heading: &str, message: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Tokenza</title><style>\
         :root{{color-scheme:light dark}}\
         body{{font-family:system-ui;text-align:center;padding:48px;background:Canvas;color:CanvasText}}\
         </style></head><body><h1>{heading}</h1><p>{message}</p></body></html>"
    )
}

fn success_page() -> String {
    page("Signed in", "You can close this tab and return to Tokenza.")
}

fn denied_page() -> String {
    page(
        "Sign-in denied",
        "Nothing was changed. Return to Tokenza and try again.",
    )
}

fn error_page() -> String {
    page("Sign-in failed", "Return to Tokenza and try again.")
}

struct Credentials {
    api_key: String,
    user_name: String,
}

#[derive(Debug)]
struct Request {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// Opens the Studio's CLI sign-in page and waits for the key it posts back.
#[tauri::command]
pub async fn commandcode_login(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let credentials = tauri::async_runtime::spawn_blocking(run_browser_flow)
        .await
        .map_err(|_| "network".to_string())??;
    let key = credentials.api_key.trim();
    if key.is_empty() {
        return Err("invalid_key".into());
    }
    if key.chars().count() > tokenza_core::secrets::MAX_LEN {
        return Err("too_long".into());
    }
    let user = tokenza_core::providers::commandcode::validate_key(key)
        .await
        .map_err(|error| match error {
            tokenza_core::ProviderError::TokenExpired
            | tokenza_core::ProviderError::NotLoggedIn => "invalid_key".to_string(),
            _ => "network".to_string(),
        })?;
    secrets::set("commandcode", "api_key", key).map_err(|e| format!("store_failed: {e}"))?;
    state.mark_secret("commandcode", "api_key", true);
    state.poll_soon("commandcode");
    scheduler::publish(&app);
    Ok(if user.trim().is_empty() {
        credentials.user_name
    } else {
        user
    })
}

fn run_browser_flow() -> Result<Credentials, String> {
    let listener = bind().ok_or_else(|| "network".to_string())?;
    let port = listener
        .local_addr()
        .map_err(|_| "network".to_string())?
        .port();
    let state = random_state()?;
    let callback = format!("http://127.0.0.1:{port}{CALLBACK_PATH}");
    let url = format!(
        "{AUTH_URL}?callback={}&state={state}&mode=redirect",
        encode_component(&callback)
    );
    open_browser(&url)?;

    let deadline = Instant::now() + TIMEOUT;
    let mut pending = None;
    let mut grace: Option<Instant> = None;
    loop {
        if grace.is_some_and(|at| Instant::now() >= at) {
            if let Some(credentials) = pending.take() {
                return Ok(credentials);
            }
            grace = None;
        }
        if Instant::now() >= deadline {
            return Err("timeout".into());
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                let Some(request) = read_request(&mut stream) else {
                    continue;
                };
                if let Some(credentials) =
                    handle_request(&mut stream, &request, &state, &mut pending, &mut grace)?
                {
                    return Ok(credentials);
                }
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(_) => return Err("network".into()),
        }
    }
}

/// Prefers the CLI's port so a firewall rule for it keeps working, then any free loopback port.
fn bind() -> Option<TcpListener> {
    for port in 5959..5969 {
        if let Ok(listener) = TcpListener::bind(("127.0.0.1", port)) {
            listener.set_nonblocking(true).ok()?;
            return Some(listener);
        }
    }
    let listener = TcpListener::bind(("127.0.0.1", 0)).ok()?;
    listener.set_nonblocking(true).ok()?;
    Some(listener)
}

fn random_state() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| "network".to_string())?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Handles one local request. `Ok(Some(_))` completes the sign-in; `Ok(None)` keeps waiting.
fn handle_request(
    stream: &mut TcpStream,
    request: &Request,
    expected_state: &str,
    pending: &mut Option<Credentials>,
    grace: &mut Option<Instant>,
) -> Result<Option<Credentials>, String> {
    let cors = cors_headers(request);
    if request.method == "OPTIONS" {
        send(stream, "204 No Content", "text/plain", &cors, "");
        return Ok(None);
    }

    let path = request.path.split('?').next().unwrap_or("/");
    if path == COMPLETE_PATH && request.method == "GET" {
        if param(&query_params(&request.path), "state") != Some(expected_state) {
            send(
                stream,
                "403 Forbidden",
                "text/html; charset=utf-8",
                &cors,
                &error_page(),
            );
            return Ok(None);
        }
        return match pending.take() {
            Some(credentials) => {
                send(
                    stream,
                    "200 OK",
                    "text/html; charset=utf-8",
                    &cors,
                    &success_page(),
                );
                Ok(Some(credentials))
            }
            None => {
                send(
                    stream,
                    "404 Not Found",
                    "text/html; charset=utf-8",
                    &cors,
                    &error_page(),
                );
                Ok(None)
            }
        };
    }
    if path != CALLBACK_PATH {
        send(
            stream,
            "404 Not Found",
            "text/html; charset=utf-8",
            &cors,
            &error_page(),
        );
        return Ok(None);
    }
    if request.method != "GET" && request.method != "POST" {
        send(
            stream,
            "405 Method Not Allowed",
            "text/html; charset=utf-8",
            &cors,
            &error_page(),
        );
        return Ok(None);
    }

    let content_type = request
        .header("content-type")
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    // The Studio has used both a query string and a request body, so accept whichever carries
    // the fields; body values win when both are present.
    let mut params = if request.method == "POST" {
        match content_type.as_str() {
            "application/x-www-form-urlencoded" => {
                pair_params(&String::from_utf8_lossy(&request.body))
            }
            "application/json" => match json_params(&request.body) {
                Some(params) => params,
                None => {
                    send(
                        stream,
                        "400 Bad Request",
                        "application/json",
                        &cors,
                        "{\"success\":false,\"error\":\"Invalid JSON\"}",
                    );
                    return Ok(None);
                }
            },
            _ if request.body.is_empty() => Vec::new(),
            _ => {
                send(
                    stream,
                    "415 Unsupported Media Type",
                    "text/html; charset=utf-8",
                    &cors,
                    &error_page(),
                );
                return Ok(None);
            }
        }
    } else {
        Vec::new()
    };
    params.extend(query_params(&request.path));

    if param(&params, "state") != Some(expected_state) {
        send(
            stream,
            "403 Forbidden",
            "text/html; charset=utf-8",
            &cors,
            &error_page(),
        );
        return Ok(None);
    }
    if let Some(error) = param(&params, "error") {
        let page = if error == "access_denied" {
            &denied_page()
        } else {
            &error_page()
        };
        send(stream, "200 OK", "text/html; charset=utf-8", &cors, page);
        return Err(if error == "access_denied" {
            "denied"
        } else {
            "failed"
        }
        .into());
    }
    let Some(api_key) = param(&params, "apiKey")
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(str::to_string)
    else {
        send(
            stream,
            "400 Bad Request",
            "text/html; charset=utf-8",
            &cors,
            &error_page(),
        );
        return Ok(None);
    };
    let credentials = Credentials {
        api_key,
        user_name: param(&params, "userName").unwrap_or_default().to_string(),
    };

    if request.method == "GET" {
        send(
            stream,
            "200 OK",
            "text/html; charset=utf-8",
            &cors,
            &success_page(),
        );
        return Ok(Some(credentials));
    }
    if content_type == "application/json" {
        send(
            stream,
            "200 OK",
            "application/json",
            &cors,
            "{\"success\":true}",
        );
        return Ok(Some(credentials));
    }

    // Form POST: the Studio expects a redirect to its "done" page, which is also when we finish.
    let mut headers = cors;
    headers.push((
        "Location",
        format!("{COMPLETE_PATH}?state={}", encode_component(expected_state)),
    ));
    *pending = Some(credentials);
    *grace = Some(Instant::now() + LANDING_GRACE);
    send(
        stream,
        "303 See Other",
        "text/html; charset=utf-8",
        &headers,
        "",
    );
    Ok(None)
}

fn cors_headers(request: &Request) -> Vec<(&'static str, String)> {
    let mut headers = vec![
        ("Access-Control-Allow-Methods", "GET, POST, OPTIONS".into()),
        ("Access-Control-Allow-Headers", "Content-Type".into()),
    ];
    if let Some(origin) = request.header("origin").filter(|origin| {
        matches!(
            *origin,
            "https://commandcode.ai" | "https://staging.commandcode.ai" | "http://localhost:3000"
        )
    }) {
        headers.push(("Access-Control-Allow-Origin", origin.to_string()));
    }
    if request.header("access-control-request-private-network") == Some("true") {
        headers.push(("Access-Control-Allow-Private-Network", "true".into()));
    }
    headers
}

fn send(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    headers: &[(&'static str, String)],
    body: &str,
) {
    let mut head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nCache-Control: no-store\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body.as_bytes());
    let _ = stream.flush();
}

fn read_request(stream: &mut TcpStream) -> Option<Request> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(end) = buffer
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .map(|at| at + 4)
        {
            break end;
        }
        if buffer.len() > MAX_HEAD {
            return None;
        }
    };

    let head = String::from_utf8_lossy(&buffer[..head_end]);
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split_whitespace();
    let method = first.next()?.to_string();
    let path = first.next()?.to_string();
    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':')?;
        headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
    }

    let length = headers
        .iter()
        .find(|(name, _)| name == "content-length")
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);
    if length > MAX_BODY {
        return None;
    }
    let mut body = buffer[head_end..].to_vec();
    while body.len() < length {
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
        if body.len() > MAX_BODY {
            return None;
        }
    }
    body.truncate(length);
    Some(Request {
        method,
        path,
        headers,
        body,
    })
}

fn query_params(path: &str) -> Vec<(String, String)> {
    path.split_once('?')
        .map(|(_, query)| pair_params(query))
        .unwrap_or_default()
}

fn pair_params(input: &str) -> Vec<(String, String)> {
    input
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (decode(key), decode(value))
        })
        .collect()
}

fn json_params(body: &[u8]) -> Option<Vec<(String, String)>> {
    let value: Value = serde_json::from_slice(body).ok()?;
    Some(
        value
            .as_object()?
            .iter()
            .filter_map(|(key, value)| Some((key.clone(), value.as_str()?.to_string())))
            .collect(),
    )
}

fn param<'a>(params: &'a [(String, String)], name: &str) -> Option<&'a str> {
    params
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

fn decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                output.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                match (hex(bytes[index + 1]), hex(bytes[index + 2])) {
                    (Some(high), Some(low)) => {
                        output.push(high * 16 + low);
                        index += 3;
                    }
                    _ => {
                        output.push(b'%');
                        index += 1;
                    }
                }
            }
            byte => {
                output.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn encode_component(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for byte in input.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            output.push(byte as char);
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
    }
    output
}

fn open_browser(url: &str) -> Result<(), String> {
    // No shell: `cmd /C start` splits the URL at `&`, and losing `state` turns the Studio page
    // into "missing required parameters". FileProtocolHandler receives the whole URL as one argument.
    #[cfg(windows)]
    let mut command = {
        let mut command = Command::new("rundll32.exe");
        command.args(["url.dll,FileProtocolHandler", url]);
        command
    };
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(all(not(windows), not(target_os = "macos")))]
    let mut command = Command::new("xdg-open");
    #[cfg(not(windows))]
    command.arg(url);
    command
        .spawn()
        .map(|_| ())
        .map_err(|_| "network".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_form_values() {
        assert_eq!(decode("a+b%2Fc"), "a b/c");
        assert_eq!(decode("100%25"), "100%");
        assert_eq!(decode("%zz"), "%zz");
        let pairs = pair_params("apiKey=abc&state=s1&userName=Nemo+King");
        assert_eq!(param(&pairs, "apiKey"), Some("abc"));
        assert_eq!(param(&pairs, "userName"), Some("Nemo King"));
    }

    #[test]
    fn reads_json_callbacks() {
        let params = json_params(
            br#"{"apiKey":"k","state":"s","userId":"u","userName":"N","keyName":"cli"}"#,
        )
        .unwrap();
        assert_eq!(param(&params, "apiKey"), Some("k"));
        assert_eq!(param(&params, "state"), Some("s"));
        assert!(json_params(b"not json").is_none());
    }

    #[test]
    fn encodes_the_callback_url() {
        assert_eq!(
            encode_component("http://127.0.0.1:5959/callback"),
            "http%3A%2F%2F127.0.0.1%3A5959%2Fcallback"
        );
    }

    #[test]
    fn state_is_unguessable() {
        let a = random_state().unwrap();
        let b = random_state().unwrap();
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
    }
}

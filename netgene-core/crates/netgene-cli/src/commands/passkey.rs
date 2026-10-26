//! One-shot local page for WebAuthn ceremonies (`http://localhost:<port>`).
//!
//! Browsers allow WebAuthn on localhost without TLS. The page offers the
//! platform authenticator or "use a phone" (QR code + Bluetooth), sends the
//! browser's response back once, and the server stops. It listens on
//! 127.0.0.1 only, under a random path, for at most five minutes; what it
//! receives is verified cryptographically by `netgene_presence::webauthn`.

use std::time::Duration;
use anyhow::{Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64URL, Engine};
use ring::rand::{SecureRandom, SystemRandom};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const TIMEOUT: Duration = Duration::from_secs(300);
const MAX_REQUEST_BYTES: usize = 64 * 1024;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);

pub enum Mode {
    Create,
    Get,
}

pub struct Ceremony {
    listener: TcpListener,
    port: u16,
    token: String,
    open_browser: bool,
}

impl Ceremony {
    pub async fn bind() -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let mut token = [0u8; 16];
        SystemRandom::new().fill(&mut token).map_err(|_| anyhow::anyhow!("RNG failure"))?;
        Ok(Self { listener, port, token: B64URL.encode(token), open_browser: true })
    }

    /// The origin the browser will report in `clientDataJSON`.
    pub fn origin(&self) -> String {
        format!("http://localhost:{}", self.port)
    }

    /// Serve the page, wait for the browser's answer and return it.
    pub async fn run(self, mode: Mode, options: serde_json::Value, title: &str) -> Result<serde_json::Value> {
        let url = format!("{}/{}", self.origin(), self.token);
        println!("🌐 Open {} in a browser on this computer", url);
        println!("   (it offers this device's authenticator or \"use a phone\" for a passkey on your phone)");
        if self.open_browser {
            open_browser(&url);
        }

        let page = page(&mode, &options, title);
        tokio::time::timeout(TIMEOUT, async {
            loop {
                let (stream, _) = self.listener.accept().await?;
                // A client that stalls is dropped instead of blocking the ceremony.
                match tokio::time::timeout(CONNECTION_TIMEOUT, self.handle(stream, &page)).await {
                    Ok(Ok(Some(answer))) => return Ok::<_, anyhow::Error>(answer),
                    Ok(Ok(None)) | Err(_) => {}
                    Ok(Err(e)) => return Err(e),
                }
            }
        })
        .await
        .context("No WebAuthn response within 5 minutes")?
    }

    async fn handle(&self, mut stream: TcpStream, page: &str) -> Result<Option<serde_json::Value>> {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        let header_end = loop {
            let n = stream.read(&mut chunk).await?;
            if n == 0 {
                return Ok(None);
            }
            buf.extend_from_slice(&chunk[..n]);
            if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                break pos + 4;
            }
            if buf.len() > MAX_REQUEST_BYTES {
                return Ok(None);
            }
        };
        let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
        let mut parts = head.split_whitespace();
        let (method, path) = (parts.next().unwrap_or_default(), parts.next().unwrap_or_default());
        let length: usize = head
            .lines()
            .find_map(|l| l.split_once(':').filter(|(k, _)| k.trim().eq_ignore_ascii_case("content-length")).map(|(_, v)| v.trim().to_string()))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
            .min(MAX_REQUEST_BYTES);
        while buf.len() < header_end + length {
            let n = stream.read(&mut chunk).await?;
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
        }

        let page_path = format!("/{}", self.token);
        let done_path = format!("/{}/done", self.token);
        match (method, path) {
            ("GET", p) if p == page_path => {
                respond(&mut stream, "200 OK", "text/html; charset=utf-8", page).await?;
                Ok(None)
            }
            ("POST", p) if p == done_path => {
                let body = buf.get(header_end..header_end + length).unwrap_or_default();
                let answer: serde_json::Value = serde_json::from_slice(body).context("Browser sent invalid JSON")?;
                respond(&mut stream, "200 OK", "text/plain; charset=utf-8", "Received. You can close this tab.").await?;
                if let Some(error) = answer.get("error").and_then(|e| e.as_str()) {
                    anyhow::bail!("Browser reported: {}", error);
                }
                Ok(Some(answer))
            }
            _ => {
                respond(&mut stream, "404 Not Found", "text/plain", "not found").await?;
                Ok(None)
            }
        }
    }
}

async fn respond(stream: &mut TcpStream, status: &str, content_type: &str, body: &str) -> Result<()> {
    let head = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        status,
        content_type,
        body.len()
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(body.as_bytes()).await?;
    stream.shutdown().await.ok();
    Ok(())
}

fn open_browser(url: &str) {
    #[cfg(windows)]
    let result = std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn();
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let result = std::process::Command::new("xdg-open").arg(url).spawn();
    if result.is_err() {
        println!("   (could not open a browser automatically)");
    }
}

fn page(mode: &Mode, options: &serde_json::Value, title: &str) -> String {
    let mode = match mode {
        Mode::Create => "create",
        Mode::Get => "get",
    };
    let title = title.replace('<', "&lt;").replace('>', "&gt;");
    // The options are JSON produced by NetGene; `</` is escaped so they cannot close the script.
    let options = options.to_string().replace("</", "<\\/");
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>NetGene — {title}</title>
<style>body{{font-family:system-ui,sans-serif;max-width:36rem;margin:3rem auto;padding:0 1rem;line-height:1.5}}button{{font-size:1.1rem;padding:.6rem 1.2rem}}#s{{margin-top:1rem;white-space:pre-wrap}}</style></head>
<body><h1>NetGene OS</h1><p>{title}</p><button id="go">Continue with a passkey</button><div id="s"></div>
<script>
const opts = {options};
const mode = "{mode}";
const dec = s => Uint8Array.from(atob(s.replace(/-/g, '+').replace(/_/g, '/') + '==='.slice((s.length + 3) % 4)), c => c.charCodeAt(0));
const enc = b => btoa(String.fromCharCode(...new Uint8Array(b))).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
const show = t => document.getElementById('s').textContent = t;
const send = async body => {{
  const r = await fetch(location.pathname + '/done', {{ method: 'POST', headers: {{ 'Content-Type': 'application/json' }}, body: JSON.stringify(body) }});
  show(await r.text());
}};
document.getElementById('go').onclick = async () => {{
  try {{
    opts.challenge = dec(opts.challenge);
    if (mode === 'create') {{
      opts.user.id = dec(opts.user.id);
      const c = await navigator.credentials.create({{ publicKey: opts }});
      await send({{ client_data_json: enc(c.response.clientDataJSON), attestation_object: enc(c.response.attestationObject) }});
    }} else {{
      opts.allowCredentials = opts.allowCredentials.map(c => ({{ ...c, id: dec(c.id) }}));
      const a = await navigator.credentials.get({{ publicKey: opts }});
      await send({{ credential_id: enc(a.rawId), client_data_json: enc(a.response.clientDataJSON), authenticator_data: enc(a.response.authenticatorData), signature: enc(a.response.signature) }});
    }}
  }} catch (e) {{
    show('Failed: ' + e);
    await send({{ error: String(e) }});
  }}
}};
</script></body></html>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn http(port: u16, request: String) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut out = String::new();
        stream.read_to_string(&mut out).await.unwrap();
        out
    }

    #[tokio::test]
    async fn test_ceremony_serves_page_and_returns_answer_once() {
        let mut ceremony = Ceremony::bind().await.unwrap();
        ceremony.open_browser = false;
        let (port, token) = (ceremony.port, ceremony.token.clone());
        let options = serde_json::json!({ "challenge": "AAAA", "allowCredentials": [] });
        let task = tokio::spawn(ceremony.run(Mode::Get, options, "Approve </script> test"));

        // A client that never finishes its request does not block the ceremony.
        let _stalled = TcpStream::connect(("127.0.0.1", port)).await.unwrap();

        let page = http(port, format!("GET /{} HTTP/1.1\r\nHost: localhost\r\n\r\n", token)).await;
        assert!(page.starts_with("HTTP/1.1 200"), "got: {page:?}");
        assert!(page.contains("navigator.credentials.get"));
        assert!(!page.contains("</script> test"), "title is escaped");

        let wrong = http(port, "POST /guess/done HTTP/1.1\r\nContent-Length: 2\r\n\r\n{}".into()).await;
        assert!(wrong.starts_with("HTTP/1.1 404"), "other paths are refused");

        let body = r#"{"credential_id":"x"}"#;
        let done = http(port, format!("POST /{}/done HTTP/1.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}", token, body.len(), body)).await;
        assert!(done.starts_with("HTTP/1.1 200"));
        assert_eq!(task.await.unwrap().unwrap()["credential_id"], "x");
        assert!(TcpStream::connect(("127.0.0.1", port)).await.is_err(), "server stops after one answer");
    }

    #[tokio::test]
    async fn test_ceremony_reports_browser_errors() {
        let mut ceremony = Ceremony::bind().await.unwrap();
        ceremony.open_browser = false;
        let (port, token) = (ceremony.port, ceremony.token.clone());
        let task = tokio::spawn(ceremony.run(Mode::Create, serde_json::json!({}), "t"));
        let body = r#"{"error":"NotAllowedError"}"#;
        http(port, format!("POST /{}/done HTTP/1.1\r\nContent-Length: {}\r\n\r\n{}", token, body.len(), body)).await;
        let err = task.await.unwrap().unwrap_err().to_string();
        assert!(err.contains("NotAllowedError"));
    }
}

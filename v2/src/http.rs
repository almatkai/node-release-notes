use reqwest::{Client, Url};
use serde_json::Value;
use std::time::Duration;
use tezbar_extension_sdk::BoxFuture;

pub const MAX_BODY: usize = 2 * 1024 * 1024;
pub trait Transport: Send + Sync + 'static {
    fn get<'a>(&'a self, url: &'a str) -> BoxFuture<'a, Result<Value, String>>;
}
pub struct Http {
    client: Client,
}
impl Http {
    pub fn new() -> Result<Self, String> {
        Self::with_timeout(Duration::from_secs(12))
    }
    pub fn with_timeout(deadline: Duration) -> Result<Self, String> {
        Ok(Self {
            client: Client::builder()
                .https_only(true)
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(4))
                .timeout(deadline)
                .pool_max_idle_per_host(1)
                .user_agent("Tezbar-Node-Release-Notes/2.0.0")
                .build()
                .map_err(|e| e.to_string())?,
        })
    }
    async fn request(&self, url: Url) -> Result<Value, String> {
        let mut response = self
            .client
            .get(url)
            .header("Accept", "application/vnd.github+json, application/json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {e}"))?;
        if !response.status().is_success() {
            return Err(response
                .status()
                .canonical_reason()
                .unwrap_or("HTTP error")
                .into());
        }
        if response
            .content_length()
            .is_some_and(|len| len > MAX_BODY as u64)
        {
            return Err("HTTP body exceeds 2097152 bytes".into());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| format!("HTTP body failed: {e}"))?
        {
            if bytes.len() + chunk.len() > MAX_BODY {
                return Err("HTTP body exceeds 2097152 bytes".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        bounded_json(&bytes)
    }
    /// Test seam uses the same request/body implementation and real TCP. It is
    /// unavailable to the production binary: no env/config endpoint overrides.
    #[cfg(test)]
    pub(crate) fn local(deadline: Duration) -> Self {
        Self {
            client: Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(deadline)
                .user_agent("Tezbar-Node-Release-Notes/2.0.0")
                .build()
                .unwrap(),
        }
    }
}
impl Transport for Http {
    fn get<'a>(&'a self, url: &'a str) -> BoxFuture<'a, Result<Value, String>> {
        Box::pin(async move {
            let url = Url::parse(url).map_err(|_| "Invalid request URL")?;
            if url.scheme() != "https"
                || url.port().is_some()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return Err("Request origin rejected".into());
            }
            let allowed = (url.host_str() == Some("nodejs.org")
                && url.path() == "/dist/index.json")
                || (url.host_str() == Some("api.github.com")
                    && url
                        .path()
                        .strip_prefix("/repos/nodejs/node/releases/tags/")
                        .is_some_and(|v| crate::model::safe_version(v).is_ok()));
            if !allowed {
                return Err("Request origin/path rejected".into());
            }
            self.request(url).await
        })
    }
}
pub fn bounded_json(bytes: &[u8]) -> Result<Value, String> {
    if bytes.len() > MAX_BODY {
        return Err("JSON body limit exceeded".into());
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|e| format!("Invalid JSON: {e}"))?;
    fn walk(v: &Value, depth: usize, count: &mut usize) -> Result<(), String> {
        *count += 1;
        if depth > 32 || *count > 200_000 {
            return Err("JSON depth/node limit exceeded".into());
        }
        match v {
            Value::String(s) if s.len() > 1_048_576 => {
                return Err("JSON string limit exceeded".into())
            }
            Value::Array(a) => {
                for v in a {
                    walk(v, depth + 1, count)?;
                }
            }
            Value::Object(o) => {
                for (k, v) in o {
                    if k.len() > 4096 {
                        return Err("JSON key limit exceeded".into());
                    }
                    walk(v, depth + 1, count)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    walk(&value, 0, &mut 0)?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    async fn mock(raw: Vec<u8>, wait: Duration) -> (Url, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let task = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut req = vec![0; 8192];
            let n = sock.read(&mut req).await.unwrap();
            tokio::time::sleep(wait).await;
            let _ = sock.write_all(&raw).await;
            String::from_utf8_lossy(&req[..n]).into_owned()
        });
        (url, task)
    }
    #[tokio::test]
    async fn real_http_bounds_status_redirect_timeout_and_headers() {
        for (raw, ok) in [
            (
                b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n[]".to_vec(),
                true,
            ),
            (
                b"HTTP/1.1 429 Too Many Requests\r\nContent-Length: 0\r\n\r\n".to_vec(),
                false,
            ),
            (
                b"HTTP/1.1 302 Found\r\nLocation: https://evil.test/\r\nContent-Length: 0\r\n\r\n"
                    .to_vec(),
                false,
            ),
            (
                b"HTTP/1.1 200 OK\r\nContent-Length: 2097153\r\n\r\n".to_vec(),
                false,
            ),
            (
                b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\n{".to_vec(),
                false,
            ),
        ] {
            let (url, task) = mock(raw, Duration::ZERO).await;
            assert_eq!(
                Http::local(Duration::from_secs(1))
                    .request(url)
                    .await
                    .is_ok(),
                ok
            );
            let headers = task.await.unwrap().to_lowercase();
            assert!(headers.contains("user-agent: tezbar-node-release-notes"));
            assert!(headers.contains("accept: application/vnd.github+json"));
        }
        let (url, task) = mock(vec![], Duration::from_millis(80)).await;
        assert!(Http::local(Duration::from_millis(20))
            .request(url)
            .await
            .is_err());
        task.await.unwrap();
        let chunk = vec![b' '; MAX_BODY + 1];
        let mut raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n".to_vec();
        raw.extend_from_slice(format!("{:x}\r\n", chunk.len()).as_bytes());
        raw.extend(chunk);
        raw.extend_from_slice(b"\r\n0\r\n\r\n");
        let (url, task) = mock(raw, Duration::ZERO).await;
        assert!(Http::local(Duration::from_secs(2))
            .request(url)
            .await
            .unwrap_err()
            .contains("body exceeds"));
        task.await.unwrap();
        assert!(bounded_json(format!("{}0{}", "[".repeat(34), "]".repeat(34)).as_bytes()).is_err());
    }
    #[tokio::test]
    async fn fixed_origins_no_override() {
        let http = Http::new().unwrap();
        for url in [
            "http://nodejs.org/dist/index.json",
            "https://evil.test/",
            "https://api.github.com/repos/nodejs/node/releases/tags/../v1.2.3",
            "https://nodejs.org/dist/index.json?x=1",
        ] {
            assert!(http.get(url).await.is_err());
        }
    }
}

#[cfg(test)]
mod tls_tests {
    use super::*;
    use std::sync::Arc;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    use tokio_rustls::{
        rustls::{
            pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer},
            ServerConfig,
        },
        TlsAcceptor,
    };
    #[tokio::test]
    async fn real_local_tls_verifies_certificate_and_bounded_body() {
        const CERT: &[u8] = include_bytes!("../qa/fixtures/localhost-test-cert.pem");
        const KEY: &[u8] = include_bytes!("../qa/fixtures/localhost-test-key.pem");
        let cert = CertificateDer::from_pem_slice(CERT).unwrap();
        let key = PrivateKeyDer::from_pem_slice(KEY).unwrap();
        let config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert], key)
            .unwrap();
        let acceptor = TlsAcceptor::from(Arc::new(config));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = Url::parse(&format!(
            "https://127.0.0.1:{}/",
            listener.local_addr().unwrap().port()
        ))
        .unwrap();
        // Owned test server: exactly two accepts, always joined, no real origins.
        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (socket, _) = listener.accept().await.unwrap();
                if let Ok(mut tls) = acceptor.accept(socket).await {
                    let mut bytes = [0; 8192];
                    let _ = tls.read(&mut bytes).await;
                    let _ = tls
                        .write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n[]",
                        )
                        .await;
                }
            }
        });
        assert!(Http::new().unwrap().request(url.clone()).await.is_err());
        let trusted = Http {
            client: Client::builder()
                .https_only(true)
                .no_proxy()
                .timeout(Duration::from_secs(2))
                .add_root_certificate(reqwest::Certificate::from_pem(CERT).unwrap())
                .build()
                .unwrap(),
        };
        assert_eq!(trusted.request(url).await.unwrap(), serde_json::json!([]));
        server.await.unwrap();
    }
    #[test]
    fn json_string_node_and_record_bounds_fail_explicitly() {
        let oversized = serde_json::to_vec(&serde_json::json!("x".repeat(1_048_577))).unwrap();
        assert!(bounded_json(&oversized)
            .unwrap_err()
            .contains("string limit"));
        let many = serde_json::to_vec(&vec![0; 200_001]).unwrap();
        assert!(bounded_json(&many).unwrap_err().contains("node limit"));
        let record = serde_json::json!({"version":"v1.2.3","date":""});
        assert!(
            crate::model::versions(serde_json::json!(vec![record; 12_001]))
                .unwrap_err()
                .contains("records")
        );
        for row in [
            serde_json::json!({"version":"x".repeat(257),"date":""}),
            serde_json::json!({"version":"v1.2.3","date":"x".repeat(129)}),
            serde_json::json!({"version":"v1.2.3","date":"","lts":"x".repeat(257)}),
        ] {
            assert!(crate::model::versions(serde_json::json!([row]))
                .unwrap_err()
                .contains("bytes"));
        }
    }
}

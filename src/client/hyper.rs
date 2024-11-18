use http::header::{CONNECTION, UPGRADE};
use http::{HeaderMap, HeaderValue, Request, Uri};
use hyper::body::Incoming;
use monoio_transports::connectors::pollio::PollIo;
use monoio_transports::connectors::{Connector, TcpConnector};
use monoio_transports::http::hyper::{HyperH1Connector, HyperH2Connector};
use monoio_transports::pool::ConnectionPool;
use std::rc::Rc;
use std::time::Duration;

use super::hyper_body::HyperBody;
use super::key::TcpAddr as Key;
use super::Protocol;
use crate::error::Error;
use crate::request::HttpRequest;

const HTTP2_SETTINGS: &str = "HTTP2-Settings";

#[derive(Default, Clone, Debug)]
struct HyperClientConfig {
    default_headers: Rc<HeaderMap>,
}

impl HyperClientConfig {
    pub fn new(header_map: HeaderMap) -> Self { HyperClientConfig { default_headers: Rc::new(header_map) } }
}

struct HyperClientInner {
    config: HyperClientConfig,
    protocol: Protocol,
    h1_connector: HyperH1Connector<PollIo<TcpConnector>, Key, HyperBody>,
    h2_connector: HyperH2Connector<PollIo<TcpConnector>, Key, HyperBody>,
}

pub struct MonoioHyperClient {
    inner: Rc<HyperClientInner>,
}

impl MonoioHyperClient {
    pub fn builder() -> HyperClientBuilder {
        HyperClientBuilder::default()
    }
}

impl Clone for MonoioHyperClient {
    fn clone(&self) -> Self {
        MonoioHyperClient {
            inner: self.inner.clone(),
        }
    }
}

#[derive(Default, Clone)]
struct HyperClientBuilderConfig {
    protocol: Protocol,
    pool_disabled: bool,
    enable_https: bool,
    default_headers: HeaderMap,
    max_idle_connections: Option<usize>,
    idle_timeout_duration: Option<Duration>,
    read_timeout: Option<Duration>,
    initial_max_streams: Option<usize>,
    max_concurrent_streams: Option<u32>,
}

#[derive(Default)]
pub struct HyperClientBuilder {
    build_config: HyperClientBuilderConfig,
}

impl HyperClientBuilder {
    pub fn disable_connection_pool(mut self) -> Self {
        self.build_config.pool_disabled = true;
        self
    }

    pub fn default_headers(mut self, val: HeaderMap) -> Self {
        self.build_config.default_headers = val;
        self
    }

    pub fn max_idle_connections(mut self, val: usize) -> Self {
        self.build_config.max_idle_connections = Some(val);
        self
    }

    pub fn idle_connections_timeout(mut self, val: u64) -> Self {
        self.build_config.idle_timeout_duration = Some(Duration::from_secs(val));
        self
    }

    pub fn read_timeout(mut self, val: u64) -> Self {
        self.build_config.read_timeout = Some(Duration::from_secs(val));
        self
    }

    pub fn initial_max_streams(mut self, val: usize) -> Self {
        self.build_config.initial_max_streams = Some(val);
        self
    }

    pub fn max_concurrent_streams(mut self, val: u32) -> Self {
        self.build_config.max_concurrent_streams = Some(val);
        self
    }

    pub fn http1_only(mut self) -> Self {
        self.build_config.protocol = Protocol::Http1;
        self
    }

    pub fn http2_prior_knowledge(mut self) -> Self {
        self.build_config.protocol = Protocol::Http2;
        self
    }

    pub fn enable_https(mut self) -> Self {
        self.build_config.enable_https = true;
        self
    }
}

impl HyperClientBuilder {
    pub fn build(&self) -> MonoioHyperClient {
        let build_config = self.build_config.clone();
        let tcp_connector = TcpConnector::default();
        let protocol_h1 = build_config.protocol.is_protocol_h1();
        let protocol_h2 = build_config.protocol.is_protocol_h2();
        let protocol_auto = build_config.protocol.is_protocol_auto();

        let config = if protocol_auto {
            // If protocol is Auto, add connection upgrade headers to default headers for every request
            let mut default_headers = build_config.default_headers.clone();
            default_headers.insert(UPGRADE, HeaderValue::from_static("h2c"));
            default_headers.insert(
                CONNECTION,
                HeaderValue::from_static("Upgrade, HTTP2-Settings"),
            );
            default_headers.insert(HTTP2_SETTINGS, HeaderValue::from_static("AAMAAABkAAQAAP__"));

            HyperClientConfig::new(default_headers)
        } else {
            HyperClientConfig::default()
        };

        // Build H1 connector with connection pool
        let h1_connector = if protocol_h1 || protocol_auto {
            let connection_pool = if build_config.pool_disabled {
                ConnectionPool::new(Some(0))
            } else {
                let idle_timeout = build_config.idle_timeout_duration;
                let max_idle = build_config.max_idle_connections;
                ConnectionPool::new_with_idle_interval(idle_timeout, max_idle)
            };

            HyperH1Connector::new_with_pool(PollIo(tcp_connector), connection_pool)
        } else {
            HyperH1Connector::new(PollIo(tcp_connector))
        };

        // Build H2 connector with connection pool
        let h2_connector = if protocol_h2 || protocol_auto {
            let connection_pool = if build_config.pool_disabled {
                ConnectionPool::new(Some(0))
            } else {
                let max_idle = build_config.max_idle_connections;
                let idle_timeout = build_config.idle_timeout_duration;
                ConnectionPool::new_with_idle_interval(idle_timeout, max_idle)
            };

            HyperH2Connector::new_with_pool(PollIo(tcp_connector), connection_pool)
        } else {
            HyperH2Connector::new(PollIo(tcp_connector))
        };

        let protocol = build_config.protocol.clone();
        let inner = Rc::new(HyperClientInner {
            config,
            protocol,
            h1_connector,
            h2_connector,
        });

        MonoioHyperClient { inner }
    }
}

impl MonoioHyperClient {
    pub fn new_request(&self) -> HttpRequest<MonoioHyperClient> {
        let mut request = HttpRequest::new(self.clone());
        for (key, val) in self.inner.config.default_headers.iter() {
            request = request.set_header(key, val)
        }

        request
    }

    pub(crate) async fn send_request(
        &self,
        req: Request<HyperBody>,
        uri: Uri,
    ) -> Result<http::Response<Incoming>, Error> {
        let key = uri.try_into().map_err(|e| Error::UriKeyError(e))?;

        let response = match self.inner.protocol {
            Protocol::Http1 => {
                let mut conn = self
                    .inner
                    .h1_connector
                    .connect(key)
                    .await
                    .map_err(|e| Error::HyperTransportError(e))?;

                conn.send_request(req).await
            }
            Protocol::Http2 => {
                let mut conn = self
                    .inner
                    .h2_connector
                    .connect(key)
                    .await
                    .map_err(|e| Error::HyperTransportError(e))?;

                conn.send_request(req).await
            }
            Protocol::Auto => {
                // First create Http/1.1 connection with upgrade headers set
                let mut conn = self
                    .inner
                    .h1_connector
                    .connect(key.clone())
                    .await
                    .map_err(|e| Error::HyperTransportError(e))?;

                let response = conn
                    .send_request(req.clone())
                    .await
                    .map_err(|e| Error::HyperResponseError(e))?;

                // Check if server response contains the upgrade header
                let should_upgrade_to_h2 = response
                    .headers()
                    .get("upgrade")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_lowercase().contains("h2c"))
                    .unwrap_or(false);

                if should_upgrade_to_h2 {
                    // Switching to H2 connection
                    let mut conn = self
                        .inner
                        .h2_connector
                        .connect(key)
                        .await
                        .map_err(|e| Error::HyperTransportError(e))?;

                    conn.send_request(req).await
                } else {
                    // Return the original H1 response
                    Ok(response)
                }
            }
        };

        response.map_err(|e| Error::HyperResponseError(e))
    }
}

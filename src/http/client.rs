use std::rc::Rc;
use std::time::Duration;

use http::{HeaderMap, Request, Uri};
use monoio::net::TcpStream;
use monoio_http::common::body::HttpBody;
use monoio_transports::connectors::TlsConnector;
use monoio_transports::connectors::{Connector, TcpConnector, TlsStream};
use monoio_transports::http::HttpConnector;

use crate::error::{Error, Result};
use crate::key::PoolKey;
use crate::Protocol;
use crate::request::HttpRequest;
use crate::response::Response;

enum HttpConnectorType {
    HTTP(HttpConnector<TcpConnector, PoolKey, TcpStream>),
    HTTPS(HttpConnector<TlsConnector<TcpConnector>, PoolKey, TlsStream<TcpStream>>),
}

#[derive(Default, Clone, Debug)]
struct ClientConfig {
    default_headers: Rc<HeaderMap>,
}

struct ClientInner {
    config: ClientConfig,
    http_connector: HttpConnectorType,
}

pub struct MonoioClient {
    inner: Rc<ClientInner>,
}

impl MonoioClient {
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }
}

impl Clone for MonoioClient {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

#[derive(Default, Clone)]
struct ClientBuilderConfig {
    protocol: Protocol,
    enable_https: bool,
    pool_disabled: bool,
    max_idle_connections: Option<usize>,
    idle_timeout_duration: Option<Duration>,
    read_timeout: Option<Duration>,
    initial_max_streams: Option<usize>,
    max_concurrent_streams: Option<u32>,
    default_headers: HeaderMap,
}

#[derive(Default)]
pub struct ClientBuilder {
    build_config: ClientBuilderConfig,
}

impl ClientBuilder {
    /// Sets default headers that will be applied to all requests made through this http.
    /// These headers can be overridden by request-specific headers.
    pub fn default_headers(mut self, val: HeaderMap) -> Self {
        self.build_config.default_headers = val;
        self
    }

    /// Disables the connection pooling feature.
    /// When disabled, a new connection will be created for each request.
    pub fn disable_connection_pool(mut self) -> Self {
        self.build_config.pool_disabled = true;
        self
    }

    /// Sets the maximum number of idle connections that can be kept in the connection pool.
    /// Once this limit is reached, older idle connections will be dropped.
    pub fn max_idle_connections(mut self, val: usize) -> Self {
        self.build_config.max_idle_connections = Some(val);
        self
    }

    /// Sets the duration after which an idle connection in the pool will be closed.
    /// The timeout is specified in seconds.
    pub fn idle_connection_timeout(mut self, val: u64) -> Self {
        self.build_config.idle_timeout_duration = Some(Duration::from_secs(val));
        self
    }

    /// Sets the read timeout for the HTTP/1.1 connections, has no effect on HTTP/2 connections
    /// After this duration elapses without receiving any data, the read operation will fail.
    /// The timeout value is specified in seconds.
    pub fn set_read_timeout(mut self, val: u64) -> Self {
        self.build_config.read_timeout = Some(Duration::from_secs(val));
        self
    }

    /// Sets the initial maximum number of streams that can be created when a new HTTP/2 connection is established.
    /// This value affects the initial stream capacity allocation and can be adjusted based on expected concurrent requests.
    pub fn initial_max_streams(mut self, val: usize) -> Self {
        self.build_config.initial_max_streams = Some(val);
        self
    }

    /// Sets the maximum number of concurrent HTTP/2 streams allowed per connection.
    /// Default is 100. Higher values allow more parallel requests on a single connection,
    /// but may require more memory and processing resources.
    pub fn max_concurrent_streams(mut self, val: u32) -> Self {
        self.build_config.max_concurrent_streams = Some(val);
        self
    }

    /// Forces the http to use HTTP/1.1 protocol only, disabling HTTP/2 support.
    /// Useful when you need to ensure HTTP/1.1 compatibility.
    pub fn http1_only(mut self) -> Self {
        self.build_config.protocol = Protocol::Http1;
        self
    }

    /// Enables HTTP/2 prior knowledge mode, assuming all connections will use HTTP/2.
    /// This skips the HTTP/1.1 -> HTTP/2 upgrade process.
    pub fn http2_prior_knowledge(mut self) -> Self {
        self.build_config.protocol = Protocol::Http2;
        self
    }

    /// Enables HTTPS/TLS support for secure connections.
    /// Must be called to make HTTPS requests.
    pub fn enable_https(mut self) -> Self {
        self.build_config.enable_https = true;
        self
    }
}

macro_rules! apply_parameter_from_config {
    ($connector:expr, $method:ident($val:expr)) => {
        match $connector {
            HttpConnectorType::HTTP(ref mut c) => c.$method($val),
            HttpConnectorType::HTTPS(ref mut c) => c.$method($val),
        }
    };

    ($connector:expr, $builder:ident().$method:ident($val:expr)) => {
        match $connector {
            HttpConnectorType::HTTP(ref mut c) => c.$builder().$method($val),
            HttpConnectorType::HTTPS(ref mut c) => c.$builder().$method($val),
        }
    };
}

impl ClientBuilder {
    pub fn build(self) -> MonoioClient {
        let build_config = self.build_config.clone();
        let config = ClientConfig::default();
        let tcp_connector = TcpConnector::default();

        let mut http_connector = if build_config.enable_https {
            // TLS implemented Connector
            // Client will negotiate the connection type using ALPN, no need to set Protocols explicitly
            let alpn = match build_config.protocol {
                Protocol::Http1 => vec!["http/1.1"],
                Protocol::Http2 => vec!["h2"],
                Protocol::Auto => vec!["http/1.1", "h2"],
            };

            let tls_connector = TlsConnector::new_with_tls_default(tcp_connector, Some(alpn));

            #[cfg(feature = "transports-patch")]
                let https_connector = HttpConnectorType::HTTPS(HttpConnector::new_with_pool_options(
                tls_connector,
                build_config.max_idle_connections,
                build_config.idle_timeout_duration,
            ));
            #[cfg(not(feature = "transports-patch"))]
                let https_connector = HttpConnectorType::HTTPS(HttpConnector::new(tls_connector));

            https_connector
        } else {
            // Default TCP Connector without TLS support
            #[cfg(not(feature = "transports-patch"))]
                let mut connector = HttpConnector::new(tcp_connector);
            #[cfg(feature = "transports-patch")]
                let mut connector = HttpConnector::new_with_pool_options(
                tcp_connector,
                build_config.max_idle_connections,
                build_config.idle_timeout_duration,
            );

            if build_config.protocol.is_protocol_h1() {
                connector.set_http1_only();
            }

            // Assumes prior http2 knowledge
            if build_config.protocol.is_protocol_h2() {
                connector.set_http2_only();
            }

            HttpConnectorType::HTTP(connector)
        };

        if let Some(val) = build_config.initial_max_streams {
            apply_parameter_from_config!(
                http_connector,
                h2_builder().initial_max_send_streams(val)
            );
        }

        if let Some(val) = build_config.max_concurrent_streams {
            apply_parameter_from_config!(http_connector, h2_builder().max_concurrent_streams(val));
        }

        apply_parameter_from_config!(http_connector, set_read_timeout(build_config.read_timeout));

        let inner = Rc::new(ClientInner {
            config,
            http_connector,
        });

        MonoioClient { inner }
    }
}

impl MonoioClient {
    /// Returns a new http request with default parameters
    pub fn make_request(&self) -> HttpRequest<MonoioClient> {
        let mut request = HttpRequest::new(self.clone());
        for (key, val) in self.inner.config.default_headers.iter() {
            request = request.set_header(key, val)
        }

        request
    }

    pub(crate) async fn send_request(
        &self,
        req: Request<HttpBody>,
        uri: Uri,
    ) -> Result<Response<HttpBody>> {
        // The connection pool keys for Non TLS and TLS based connectors slightly differ
        let key = uri.try_into().map_err(|e| Error::UriKeyError(e))?;
        let (response, _) = match self.inner.http_connector {
            HttpConnectorType::HTTP(ref connector) => {
                let mut conn = connector
                    .connect(key)
                    .await
                    .map_err(|e| Error::HttpTransportError(e))?;
                conn.send_request(req).await
            }

            HttpConnectorType::HTTPS(ref connector) => {
                let mut conn = connector
                    .connect(key)
                    .await
                    .map_err(|e| Error::HttpTransportError(e))?;
                conn.send_request(req).await
            }
        };

        response.map_err(|e| Error::HttpResponseError(e))
    }
}

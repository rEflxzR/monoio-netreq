use std::net::ToSocketAddrs;

// Borrowed from TcpTlsAddrs
use http::Uri;
use service_async::Param;
use monoio_transports::connectors::ServerName;
use monoio_transports::FromUriError;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PoolKey {
    pub host: smol_str::SmolStr,
    pub port: u16,
    pub sn: ServerName<'static>,
}

impl Param<ServerName<'static>> for PoolKey {
    #[inline]
    fn param(&self) -> ServerName<'static> {
        self.sn.clone()
    }
}

impl AsRef<ServerName<'static>> for PoolKey {
    #[inline]
    fn as_ref(&self) -> &ServerName<'static> {
        &self.sn
    }
}

impl ToSocketAddrs for PoolKey {
    type Iter = <(&'static str, u16) as ToSocketAddrs>::Iter;

    #[inline]
    fn to_socket_addrs(&self) -> std::io::Result<Self::Iter> {
        (self.host.as_str(), self.port).to_socket_addrs()
    }
}

impl TryFrom<&Uri> for PoolKey {
    type Error = FromUriError;

    #[inline]
    fn try_from(uri: &Uri) -> Result<Self, Self::Error> {
        let host = match uri.host() {
            Some(a) => a,
            None => return Err(FromUriError::NoAuthority),
        };

        let (tls, default_port) = match uri.scheme() {
            Some(scheme) if scheme == &http::uri::Scheme::HTTP => (false, 80),
            Some(scheme) if scheme == &http::uri::Scheme::HTTPS => (true, 443),
            _ => (false, 0),
        };
        if (tls && default_port != 443) || (!tls && default_port == 443) {
            return Err(FromUriError::UnsupportScheme);
        }
        let host = smol_str::SmolStr::from(host);
        let port = uri.port_u16().unwrap_or(default_port);

        let sn = {
            #[cfg(any(feature = "native-tls", feature = "native-tls-patch"))]
            {
                host.as_str().into()
            }
            #[cfg(all(not(feature = "native-tls"), not(feature = "native-tls-patch")))]
            {
                ServerName::try_from(host.to_string())?
            }
        };

        Ok(PoolKey { host, port, sn })
    }
}

impl TryFrom<Uri> for PoolKey {
    type Error = FromUriError;

    #[inline]
    fn try_from(value: Uri) -> Result<Self, Self::Error> {
        Self::try_from(&value)
    }
}

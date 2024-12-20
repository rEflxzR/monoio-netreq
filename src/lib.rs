#[cfg(not(feature = "hyper-tls"))]
pub mod http;
mod request;
mod response;
mod error;
#[cfg(any(feature = "hyper", feature = "pool-hyper", feature = "hyper-tls"))]
pub mod hyper;
mod key;

#[derive(Default, Clone, PartialEq, Debug)]
enum Protocol {
    Http1,
    Http2,
    #[default]
    Auto,
}

impl Protocol {
    pub(crate) fn is_protocol_h1(&self) -> bool {
        self.eq(&Protocol::Http1)
    }

    pub(crate) fn is_protocol_h2(&self) -> bool {
        self.eq(&Protocol::Http2)
    }

    #[allow(dead_code)]
    pub(crate) fn is_protocol_auto(&self) -> bool {
        self.eq(&Protocol::Auto)
    }
}
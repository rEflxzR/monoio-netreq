// Alias for the patched crate
#[cfg(feature = "transports-patch")]
extern crate monoio_transports_patched as monoio_transports;

pub mod http;
mod request;
mod response;
mod error;
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
        match self {
            Protocol::Http1 => true,
            _ => false,
        }
    }

    pub(crate) fn is_protocol_h2(&self) -> bool {
        match self {
            Protocol::Http2 => true,
            _ => false,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn is_protocol_auto(&self) -> bool {
        match self {
            Protocol::Auto => true,
            _ => false,
        }
    }
}
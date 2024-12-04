#[cfg(test)]
mod test {
    #[allow(unused_imports)]
    use bytes::Bytes;
    use http::{Method, Version};
    use monoio_netreq::http::client::MonoioClient;
    #[cfg(any(feature = "hyper", feature = "hyper-patch"))]
    use monoio_netreq::hyper::client::MonoioHyperClient;

    const BODY: &str = r#"{"data": {"name": "FNS"}}"#;

    #[monoio::test(driver = "legacy", timer = true)]
    async fn http1_tls_client() -> anyhow::Result<()> {
        let client = MonoioClient::builder()
            .max_idle_connections(5)
            .idle_connection_timeout(5)
            .enable_https()
            .http1_only()
            .build();
        let http_result = client
            .make_request()
            .set_method(Method::GET)
            .set_uri("https://httpbin.org/ip")
            .set_header("Content-Type", "application/json")
            .set_version(Version::HTTP_11)
            .send()
            .await?;

        let res = http_result;
        assert_eq!(res.status(), 200);
        assert_eq!(res.version(), Version::HTTP_11);

        Ok(())
    }

    #[monoio::test(driver = "legacy", timer = true)]
    async fn http2_tls_client() -> anyhow::Result<()> {
        let client = MonoioClient::builder()
            .max_idle_connections(5)
            .idle_connection_timeout(5)
            .enable_https()
            .http2_prior_knowledge()
            .build();
        let url = "https://httpbin.org/post";
        let body = Bytes::from_static(BODY.as_ref());
        let http_result = client
            .make_request()
            .set_method(Method::POST)
            .set_uri(url)
            .set_header("Content-type", "application/json")
            .set_version(Version::HTTP_2)
            .send_body(body.clone())
            .await?;

        let res = http_result;
        assert_eq!(res.status(), 200);
        assert_eq!(res.version(), http::Version::HTTP_2);

        Ok(())
    }

    #[monoio::test(driver = "legacy", timer = true)]
    // This http sets the Protocol as Auto
    async fn alpn_auto_tls_client() -> anyhow::Result<()> {
        let client = MonoioClient::builder()
            .max_idle_connections(5)
            .idle_connection_timeout(5)
            .enable_https()
            .build();
        let http_result = client
            .make_request()
            .set_method(Method::GET)
            .set_uri("https://httpbin.org/ip")
            .set_header("Content-Type", "application/json")
            .send()
            .await?;

        let res = http_result;
        assert_eq!(res.status(), 200);
        assert_eq!(res.version(), Version::HTTP_2);

        Ok(())
    }

    #[monoio::test(driver = "legacy", timer = true)]
    async fn http1_non_tls_client() -> anyhow::Result<()> {
        let client = MonoioClient::builder()
            .max_idle_connections(5)
            .idle_connection_timeout(5)
            .http1_only()
            .build();
        let http_result = client
            .make_request()
            .set_method(Method::GET)
            .set_uri("http://nghttp2.org/httpbin/ip")
            .set_header("Content-Type", "application/json")
            .set_version(Version::HTTP_11)
            .send()
            .await?;

        let res = http_result;
        assert_eq!(res.status(), 200);
        assert_eq!(res.version(), Version::HTTP_11);

        Ok(())
    }

    #[monoio::test(driver = "legacy", timer = true)]
    async fn http2_non_tls_client() -> anyhow::Result<()> {
        let client = MonoioClient::builder()
            .max_idle_connections(5)
            .idle_connection_timeout(5)
            .http2_prior_knowledge()
            .build();
        let body = Bytes::from_static(BODY.as_ref());
        let http_result = client
            .make_request()
            .set_method(Method::POST)
            .set_uri("http://nghttp2.org/httpbin/post")
            .set_header("Content-Type", "application/json")
            .set_version(Version::HTTP_2)
            .send_body(body)
            .await?;

        let res = http_result;
        assert_eq!(res.status(), 200);
        assert_eq!(res.version(), Version::HTTP_2);

        Ok(())
    }

    #[cfg(any(feature = "hyper", feature = "hyper-patch"))]
    #[monoio::test(driver = "legacy", timer = true)]
    async fn hyper_http1_non_tls_client() -> anyhow::Result<()> {
        let client = MonoioHyperClient::builder()
            .max_idle_connections(5)
            .idle_connection_timeout(5)
            .http1_only()
            .build();
        let body = Bytes::from(BODY);
        let http_result = client
            .new_request()
            .set_method(Method::GET)
            .set_uri("http://nghttp2.org/httpbin/ip")
            .set_header("Content-Type", "application/json")
            .set_version(Version::HTTP_11)
            .send_body(body)
            .await?;

        assert_eq!(http_result.status(), 200);
        assert_eq!(http_result.version(), Version::HTTP_11);

        Ok(())
    }

    #[cfg(any(feature = "hyper", feature = "hyper-patch"))]
    #[monoio::test(driver = "legacy", timer = true)]
    async fn hyper_http2_non_tls_client() -> anyhow::Result<()> {
        let client = MonoioHyperClient::builder()
            .max_idle_connections(5)
            .idle_connection_timeout(5)
            .http2_prior_knowledge()
            .build();
        let body = Bytes::from(BODY);
        let http_result = client
            .new_request()
            .set_method(Method::GET)
            .set_uri("http://nghttp2.org/httpbin/ip")
            .set_header("Content-Type", "application/json")
            .set_version(Version::HTTP_2)
            .send_body(body)
            .await?;

        assert_eq!(http_result.status(), 200);
        assert_eq!(http_result.version(), Version::HTTP_2);

        Ok(())
    }

    #[cfg(any(feature = "hyper", feature = "hyper-patch"))]
    #[monoio::test(driver = "legacy", timer = true)]
    async fn hyper_non_tls_client() -> anyhow::Result<()> {
        let client = MonoioHyperClient::builder()
            .max_idle_connections(5)
            .idle_connection_timeout(5)
            .build();
        let http_result = client
            .new_request()
            .set_method(Method::GET)
            .set_uri("http://nghttp2.org/httpbin/ip")
            .set_header("Content-Type", "application/json")
            .set_version(Version::HTTP_11)
            .send()
            .await?;

        assert_eq!(http_result.status(), 200);
        // Server accepted connection upgrade to HTTP_2 from HTTP_11
        assert_eq!(http_result.version(), Version::HTTP_2);

        Ok(())
    }
}
use crate::{
    config::engine::connect::{ConnectRequest, IoWrapper},
    core::endpoint::Endpoint,
    Result,
};
use anyhow::Context;
use http_body_util::{BodyExt, Empty};
use hyper::{Method, Request};
use hyper_util::rt::TokioIo;
use maxminddb::{geoip2::Country, Mmap, Reader};
use reqwest::Url;
use rune::{
    runtime::{Function, Ref},
    Any,
};
use sha2::{Digest, Sha256};
use std::{
    env,
    fs::{self, create_dir_all},
    net::IpAddr,
    rc::Rc,
    time::Duration,
};
use tracing::{debug, info};

#[derive(Any, Debug, Clone)]
pub struct GeoIp {
    reader: Rc<Reader<Mmap>>,
}

#[rune::function]
pub fn create_geoip_from_absolute_path(path: Ref<str>) -> Result<GeoIp> {
    let reader = Reader::open_mmap(path.as_ref())
        .with_context(|| format!("Failed to load GeoIP database from {}", path.as_ref()))?;

    Ok(GeoIp {
        reader: Rc::new(reader),
    })
}

#[rune::function(path = create_geoip_from_url_async)]
pub async fn create_geoip_from_url(
    url: Ref<str>,
    handler: Function,
    update_interval: u64,
) -> Result<GeoIp> {
    // First create the temp directory if it doesn't exist
    let temp_dir = env::temp_dir().join("dandelion/geoip");
    create_dir_all(&temp_dir).context("Failed to create temp directory")?;

    // now create a filename using the hash of the url
    let mut hasher = Sha256::new();
    hasher.update(url.as_ref().as_bytes());
    let url_hash = format!("{:x}", hasher.finalize());
    let db_path = temp_dir.join(format!("{}.mmdb", url_hash));

    // If the file already exists, we check if it is older than the update interval
    if db_path.exists() {
        let metadata = fs::metadata(&db_path).context("Failed to read metadata")?;
        let modified = metadata.modified().context("Failed to get modified time")?;

        if modified.elapsed().context("Failed to get elapsed time")?
            < Duration::from_secs(update_interval)
        {
            info!("Using cached GeoIP database from {}", db_path.display());

            let reader =
                Reader::open_mmap(&db_path).context("Failed to open existing GeoIP database")?;

            return Ok(GeoIp {
                reader: Rc::new(reader),
            });
        }
    }

    info!(
        "Downloading GeoIP database from {} to {}",
        url.as_ref(),
        db_path.display()
    );

    let url = Url::parse(url.as_ref()).context("Failed to parse GeoIP database URL")?;
    if url.scheme() != "https" && url.scheme() != "http" {
        anyhow::bail!("Unsupported URL scheme: {}", url.scheme());
    }

    let domain = url.domain().ok_or_else(|| {
        anyhow::anyhow!("GeoIP database URL must have a domain: {}", url.as_ref())
    })?;

    let port = url
        .port()
        .unwrap_or(if url.scheme() == "https" { 443 } else { 80 });

    let io = handler
        .async_send_call::<(ConnectRequest,), Result<IoWrapper>>((ConnectRequest::new(
            Endpoint::new_from_domain(domain, port),
        ),))
        .await
        .into_result()??
        .into_inner();

    let (mut request_sender, connection) =
        hyper::client::conn::http1::handshake(TokioIo::new(io)).await?;

    let connection_task = tokio::task::spawn_local(async move {
        if let Err(err) = connection.await {
            if err.is_canceled() {
                return;
            }

            debug!("Connection to download GeoIP failed: {:?}", err);
        }
    });

    let req = Request::builder()
        .method(Method::GET)
        .uri(url.as_ref())
        .header("Connection", "close")
        .header("Host", domain)
        .body(Empty::<hyper::body::Bytes>::new())?;

    let response = request_sender.send_request(req).await?;

    if response.status() != 200 {
        anyhow::bail!(
            "HTTP request failed: {}, {}",
            response.status(),
            String::from_utf8_lossy(&response.collect().await?.to_bytes())
        );
    }

    let body = response.collect().await?.to_bytes();

    // Force abort the connection task since we're done with the response
    connection_task.abort();

    fs::write(&db_path, body).context("Failed to write GeoIP database")?;
    info!("Downloaded GeoIP database to {}", db_path.display());

    let reader = Reader::open_mmap(&db_path).context("Failed to open downloaded GeoIP database")?;

    Ok(GeoIp {
        reader: Rc::new(reader),
    })
}

impl GeoIp {
    // We don't differentiate any error here, just return an empty string.
    // User should not care about the internal implementation of maxminddb.
    #[rune::function]
    pub fn lookup(&self, ip: &str) -> String {
        let ip: IpAddr = match ip.parse() {
            Ok(ip) => ip,
            Err(_) => return "".to_owned(),
        };

        match self.reader.lookup::<Country>(ip) {
            Ok(country) => country
                .and_then(|c| c.country)
                .and_then(|c| c.iso_code)
                .unwrap_or(""),
            Err(_) => "",
        }
        .to_owned()
    }

    pub fn module() -> Result<rune::Module> {
        let mut module = rune::Module::new();

        module.ty::<Self>()?;

        module.function_meta(Self::lookup)?;
        module.function_meta(create_geoip_from_url)?;
        module.function_meta(create_geoip_from_absolute_path)?;

        Ok(module)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::engine::{connect::ConnectRequest, resolver::ResolverWrapper, testing};

    #[tokio::test]
    async fn test_geoip_from_absolute_path() -> Result<()> {
        let _: GeoIp = testing::run(
            vec![GeoIp::module()?],
            &format!(
                "Ok(create_geoip_from_absolute_path(\"{}\")?)",
                // join the path to the current directory
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("fixtures/asn-country.mmdb")
                    .to_string_lossy()
            ),
            ((),),
        )
        .await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_geoip_from_url() -> Result<()> {
        let local_set = tokio::task::LocalSet::new();

        let _: GeoIp = local_set.run_until(async {
            anyhow::Ok::<GeoIp>(testing::run(
                            vec![GeoIp::module()?, ResolverWrapper::module()?, ConnectRequest::module()?],
                            r#"
                                async fn handle(endpoint) {
                                    let resolver = create_system_resolver()?;
                                    Ok(new_tls_async(endpoint.endpoint(), new_tcp_async(endpoint.endpoint(), resolver).await?).await?)
                                }

                                Ok(create_geoip_from_url_async("https://cdn.jsdelivr.net/npm/@ip-location-db/asn-country-mmdb/asn-country-ipv4.mmdb", handle, 3600).await?)
                            "#,
                            ((), ),
                        )
                        .await?)
        }).await?;

        Ok(())
    }
}

use crate::{config::engine::connect::IoWrapper, Result};
use anyhow::Context;
use flate2::read::GzDecoder;
use http_body_util::{BodyExt, Empty};
use hyper::{Method, Request};
use hyper_util::rt::TokioIo;
use maxminddb::{geoip2::Country, Mmap, Reader};
use rune::{
    runtime::{Function, Ref},
    Any,
};
use std::{
    env,
    fs::{create_dir_all, read_dir},
    net::IpAddr,
    sync::Arc,
};
use tar::Archive;
use tempfile::tempdir;
use tracing::{debug, info};

#[derive(Any, Debug, Clone)]
pub struct GeoIp {
    reader: Arc<Reader<Mmap>>,
}

impl GeoIp {
    #[rune::function(path = Self::from_absolute_path)]
    pub fn from_absolute_path(path: Ref<str>) -> Result<Self> {
        let reader = Reader::open_mmap(path.as_ref())
            .with_context(|| format!("Failed to load GeoIP database from {}", path.as_ref()))?;

        Ok(GeoIp {
            reader: Arc::new(reader),
        })
    }

    #[rune::function(path = Self::from_license)]
    pub async fn from_license(
        license: Ref<str>,
        handler: Function,
        force_update: bool,
    ) -> Result<Self> {
        let temp_dir = env::temp_dir().join("dandelion/geoip");
        let db_path = temp_dir.join("GeoLite2-Country.mmdb");

        if !force_update {
            // first try to load the file
            if let Ok(reader) = Reader::open_mmap(&db_path) {
                debug!(
                    "Found existing GeoList2 database from {}",
                    db_path.to_str().unwrap()
                );
                return Ok(Self {
                    reader: Arc::new(reader),
                });
            }
        }

        let dir = tempdir()?;

        info!(
            "Downloading GeoLite2 database from remote to temp folder {} ...",
            dir.path().to_str().unwrap()
        );

        let io = handler
            .async_send_call::<(String,), IoWrapper>(("download.maxmind.com:443".to_owned(),))
            .await
            .into_result()?
            .into_inner();

        let path = format!(
            "/app/geoip_download?edition_id=GeoLite2-Country&license_key={}&suffix=tar.gz",
            license.as_ref()
        );

        let (mut request_sender, connection) =
            hyper::client::conn::http1::handshake(TokioIo::new(io)).await?;

        let connection_task = tokio::task::spawn(async move {
            if let Err(err) = connection.await {
                if err.is_canceled() {
                    return;
                }

                debug!("Connection to download GeoIP failed: {:?}", err);
            }
        });

        let req = Request::builder()
            .method(Method::GET)
            .uri(format!("https://download.maxmind.com{}", path))
            .header("Host", "download.maxmind.com")
            .header("User-Agent", "dandelion/1.0")
            .header("Connection", "close")
            .body(Empty::<hyper::body::Bytes>::new())?;

        let response = request_sender.send_request(req).await?;

        if response.status() != 200 {
            return Err(anyhow::anyhow!(
                "HTTP request failed: {}",
                response.status()
            ));
        }

        let body = response.collect().await?.to_bytes();

        // Force abort the connection task since we're done with the response
        connection_task.abort();

        let tar = GzDecoder::new(body.as_ref());
        let mut archive = Archive::new(tar);
        archive.unpack(&dir)?;

        // The file is extracted to a folder with the release data of
        // the database

        // We first try to find the folder
        let db_temp_dir = read_dir(&dir)?
            .filter_map(|e| e.ok())
            .find(|e| e.path().is_dir())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Failed to find the downloaded file. Maxmind changed the archive structure?"
                )
            })?
            .path();

        create_dir_all(db_path.parent().unwrap())?;

        std::fs::copy(db_temp_dir.join("GeoLite2-Country.mmdb"), &db_path)?;
        info!("Downloaded GeoLite2 database");

        Ok(Self {
            reader: Arc::new(Reader::open_mmap(&db_path)?),
        })
    }

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
        module.function_meta(Self::from_absolute_path)?;
        module.function_meta(Self::from_license)?;

        Ok(module)
    }
}

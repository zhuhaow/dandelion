use flate2::read::GzDecoder;
use maxminddb::{geoip2::country::Country, Mmap, Reader};
use reqwest::ClientBuilder;
use rune::Any;
use specht_core::Result;
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
    pub fn from_absolute_path(path: &str) -> Result<Self> {
        let reader = Reader::open_mmap(path)?;
        Ok(Self {
            reader: Arc::new(reader),
        })
    }

    pub async fn from_license(license: &str) -> Result<Self> {
        let temp_dir = env::temp_dir().join("specht2/geoip");
        let db_path = temp_dir.join("GeoLite2-Country.mmdb");

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

        let dir = tempdir()?;

        info!(
            "Downloading GeoLite2 database from remote to temp folder {} ...",
            dir.path().to_str().unwrap()
        );
        let url = format!("https://download.maxmind.com/app/geoip_download?edition_id=GeoLite2-Country&license_key={}&suffix=tar.gz", license);
        let response = ClientBuilder::new()
            .no_proxy()
            .build()?
            .get(url)
            .send()
            .await?;
        let slice = &response.bytes().await?[..];

        let tar = GzDecoder::new(slice);
        let mut archive = Archive::new(tar);
        archive.unpack(&dir)?;

        // The file is extracted to a folder with the release data of
        // the database, so it's super tedious to use.

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
    pub fn lookup(&self, ip: &str) -> String {
        ip.parse::<IpAddr>()
            .map(|ip| match self.reader.lookup::<Country>(ip) {
                Ok(country) => country.iso_code.unwrap_or(""),
                Err(_) => "",
            })
            .map(|s| s.to_owned())
            .unwrap_or_else(|_| "".to_owned())
    }

    pub fn module() -> Result<rune::Module> {
        let mut module = rune::Module::new();

        module.ty::<Self>()?;
        module.inst_fn("lookup", Self::lookup)?;

        module.async_function(["try_geoip_from_license_async"], Self::from_license)?;
        module.function(["try_geoip_from_absolute_path"], Self::from_absolute_path)?;

        Ok(module)
    }
}

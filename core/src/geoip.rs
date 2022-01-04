use crate::Result;
use flate2::read::GzDecoder;
use log::debug;
use maxminddb::Reader;
use memmap2::Mmap;
use serde::Deserialize;
use std::{
    env,
    fs::{create_dir_all, read_dir},
    path::{Path, PathBuf},
};
use tar::Archive;
use tempfile::tempdir;

#[derive(Debug, Deserialize, Clone)]
pub enum Source {
    File(PathBuf),
    License(String),
}

pub async fn create_reader(source: &Source) -> Result<Reader<Mmap>> {
    match source {
        Source::File(p) => Reader::open_mmap(p).map_err(Into::into),
        Source::License(license) => {
            // Create a temp folder first.
            let tempdir = env::temp_dir().join("specht2/geoip");
            let db_path = tempdir.join("GeoLite2-Country.mmdb");

            ensure_reader(license, db_path.as_path()).await
        }
    }
}

async fn download_db(license: &str, to: &Path) -> Result<()> {
    let dir = tempdir()?;

    debug!(
        "Downloading GeoLite2 database from remote to temp folder {} ...",
        dir.path().to_str().unwrap()
    );
    let url = format!("https://download.maxmind.com/app/geoip_download?edition_id=GeoLite2-Country&license_key={}&suffix=tar.gz", license);
    let response = reqwest::ClientBuilder::new()
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

    create_dir_all(to.parent().unwrap())?;

    std::fs::copy(db_temp_dir.join("GeoLite2-Country.mmdb"), &to)?;
    debug!("Done");

    Ok(())
}

fn open_reader(from: &Path) -> Result<Reader<Mmap>> {
    Reader::open_mmap(from).map_err(Into::into)
}

async fn ensure_reader(license: &str, temp_file: &Path) -> Result<Reader<Mmap>> {
    // first try to load the file
    if let Ok(reader) = open_reader(temp_file) {
        return Ok(reader);
    }

    download_db(license, temp_file).await?;

    open_reader(temp_file)
}

#[cfg(test)]
mod tests {
    use maxminddb::geoip2::Country;
    use tempfile::NamedTempFile;

    use super::*;

    #[test_log::test(tokio::test)]
    #[ignore]
    async fn bootstrap_from_license() -> Result<()> {
        let license = env::var("MAXMINDDB_LICENSE")?;
        // We skip test when we explicitly disable it.
        if license == "DISABLED" {
            return Ok(());
        }

        let temp = NamedTempFile::new()?;
        let builder = ensure_reader(&license, temp.path()).await?;

        let result: Country = builder.lookup("8.8.8.8".parse().unwrap())?;

        assert_eq!(result.country.unwrap().iso_code.unwrap(), "US");

        Ok(())
    }
}

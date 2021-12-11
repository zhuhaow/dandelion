use crate::Result;
use anyhow::Context;
use flate2::read::GzDecoder;
use log::info;
use maxminddb::Reader;
use memmap2::Mmap;
use std::{
    env,
    fs::{create_dir_all, read_dir},
    path::PathBuf,
};
use tar::Archive;

pub enum Source {
    File(PathBuf),
    License(String),
}

pub async fn create_builder(source: Source) -> Result<Reader<Mmap>> {
    match source {
        Source::File(p) => Reader::open_mmap(p).map_err(Into::into),
        Source::License(license) => {
            // Create a temp folder first.
            let tempdir = env::temp_dir().join("specht2/geoip");
            let db_path = tempdir.join("GeoLite2-Country.mmdb");

            create_dir_all(&tempdir)
                .context("Failed to create temp folder to hold GeoLite database file")?;

            // For now we only download database if there is no one existing

            if !db_path.exists() {
                info!(
                    "Downloading GeoLite2 database from remote to {} ...",
                    db_path.to_str().unwrap()
                );
                let url = format!("https://download.maxmind.com/app/geoip_download?edition_id=GeoLite2-Country&license_key={}&suffix=tar.gz", license);
                let response = reqwest::get(url).await?;
                let slice = &response.bytes().await?[..];

                let tar = GzDecoder::new(slice);
                let mut archive = Archive::new(tar);
                archive.unpack(&tempdir)?;

                // The file is extracted to a folder with the release data of
                // the database, so it's super tedious to use.

                // We first try to find the folder
                let db_temp_dir = read_dir(tempdir)?
                    .filter_map(|e| e.ok())
                    .find(|e| e.path().is_dir())
                    .ok_or_else(|| anyhow::anyhow!("Failed to find the downloaded file. Maxmind changed the archive structure?"))?.path();

                std::fs::copy(db_temp_dir.join("GeoLite2-Country.mmdb"), &db_path)?;
                std::fs::remove_dir_all(db_temp_dir)?;

                info!("Done");
            }

            Reader::open_mmap(db_path).map_err(Into::into)
        }
    }
}

#[cfg(test)]
mod tests {
    use maxminddb::geoip2::Country;

    use super::*;

    #[test_log::test(tokio::test)]
    async fn test_name() -> Result<()> {
        let license = env::var("MAXMINDDB_LICENSE")?;
        let builder = create_builder(Source::License(license)).await?;

        let result: Country = builder.lookup("8.8.8.8".parse().unwrap())?;

        assert_eq!(result.country.unwrap().iso_code.unwrap(), "US");

        Ok(())
    }
}

use crate::{
    geoip::{create_reader, Source},
    Result,
};
use maxminddb::Reader;
use memmap2::Mmap;
use std::sync::Arc;

pub struct GeoIpBuilder {
    source: Source,
    reader: Option<Arc<Reader<Mmap>>>,
}

impl GeoIpBuilder {
    pub fn new(source: Source) -> Self {
        Self {
            source,
            reader: None,
        }
    }

    pub async fn get(&mut self) -> Result<Arc<Reader<Mmap>>> {
        if self.reader.is_none() {
            self.reader = Some(Arc::new(create_reader(&self.source).await?));
        }

        Ok(self.reader.as_ref().unwrap().clone())
    }
}

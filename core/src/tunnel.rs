use crate::{io::Io, Result};
use tokio::io::copy_bidirectional;

pub struct Tunnel {
    local: Box<dyn Io>,
    remote: Box<dyn Io>,
}

impl Tunnel {
    pub fn new(local: Box<dyn Io>, remote: Box<dyn Io>) -> Self {
        Self { local, remote }
    }

    pub async fn process(&mut self) -> Result<()> {
        copy_bidirectional(&mut self.local, &mut self.remote).await?;

        Ok(())
    }
}

use crate::{io::Io, Result};
use tokio::io::copy_bidirectional;

pub async fn tunnel(mut left: impl Io, mut right: impl Io) -> Result<()> {
    copy_bidirectional(&mut left, &mut right).await?;

    Ok(())
}

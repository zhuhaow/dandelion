use anyhow::Context;
use specht2_core::{
    server::{Server, ServerConfig},
    Result,
};
use std::{
    env,
    fs::read_to_string,
    path::{Path, PathBuf},
};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "specht2", about = "CLI version of the Specht2 client")]
struct Opt {
    #[structopt(parse(from_os_str))]
    input: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    flexi_logger::Logger::try_with_env()
        .unwrap()
        .start()
        .unwrap();

    let opt: Opt = Opt::from_args();

    let config: ServerConfig = match opt.input {
        Some(path) => ron::de::from_str(&read_to_string(path)?)?,
        None => {
            // Try to load from Snap common data directory
            let path = env::var("SNAP_COMMON")
                .map(|p| Path::new(&p).join("config.ron"))
                .with_context(|| "Failed to load config file from $SNAP_COMMON")?;

            ron::de::from_str(&read_to_string(path)?)?
        }
    };

    Server::new(config).serve().await?;
    Ok(())
}

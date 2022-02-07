use futures::future::AbortHandle;
use specht_core::Result;
use specht_server::{config::ServerConfig, privilege::NoPrivilegeHandler, Server};
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
    flexi_logger::Logger::try_with_env_or_str("warn,specht_core=info")
        .unwrap()
        .start()
        .unwrap();

    #[cfg(not(target_os = "windows"))]
    {
        use fdlimit::raise_fd_limit;
        use tracing::info;
        use tracing::warn;

        match raise_fd_limit() {
            Some(limit) => info!("Raised fd limit to {}", limit),
            None => warn!("Failed to raise fd limit, this may cause \"Too many files error\" when there is too many connections"),
        }
    }

    let opt: Opt = Opt::from_args();

    let path: PathBuf = opt
        .input
        .or_else(|| {
            env::var("SNAP_COMMON")
                .map(|p| Path::new(&p).join("config.ron"))
                .ok()
        })
        .or_else(|| {
            env::var("HOME")
                .map(|p| Path::new(&p).join("./.specht2/config.ron"))
                .ok()
        })
        .ok_or(anyhow::anyhow!(
            "Failed to load config file from $SNAP_COMMON and $HOME"
        ))?;

    let config: ServerConfig = ron::de::from_str(&read_to_string(path)?)?;
    let (_, reg) = AbortHandle::new_pair();

    Server::new(config, NoPrivilegeHandler::default())
        .serve(reg)
        .await?;

    Ok(())
}
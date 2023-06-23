use anyhow::{Context, Ok};
use specht_config::Instance;
use specht_core::Result;
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
    flexi_logger::Logger::try_with_env_or_str("warn,specht_core=info,specht_config=info")
        .unwrap()
        .start()
        .unwrap();

    #[cfg(not(target_os = "windows"))]
    {
        use fdlimit::raise_fd_limit;
        use tracing::{info, warn};

        match raise_fd_limit() {
            Some(limit) => info!("Raised fd limit to {}", limit),
            None => warn!("Failed to raise fd limit, this may cause \"Too many files error\" when there is too many connections"),
        }
    }

    let opt: Opt = Opt::from_args();

    fn load_config_from_env(env: &str, path: &str) -> Result<String> {
        Ok(read_to_string(Path::new(&env::var(env)?).join(path))?)
    }

    let code = match opt.input {
        Some(path) => read_to_string(&path)
            .with_context(|| format!("Failed to load config file {}", path.to_string_lossy()))?,
        None => load_config_from_env("SNAP_COMMON", "./config.rn")
            .or_else(|_| load_config_from_env("HOME", "./.specht2/config.rn"))
            .context(
                "Failed to load config from $SNAP_COMMON/config.rn or $HOME/.specht2/config.rn",
            )?,
    };

    let instance = Instance::load_config("config", code).await?;

    instance.run().await
}

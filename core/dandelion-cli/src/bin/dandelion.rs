use anyhow::Context;
use dandelion_config::Engine;
use dandelion_core::Result;
use fdlimit::Outcome;
use std::{
    env,
    fs::read_to_string,
    path::{Path, PathBuf},
};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "dandelion", about = "CLI version of the dandelion client")]
struct Opt {
    #[structopt(parse(from_os_str))]
    input: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    flexi_logger::Logger::try_with_env_or_str("warn,dandelion_core=info,dandelion_config=info")
        .unwrap()
        .start()
        .unwrap();

    #[cfg(not(target_os = "windows"))]
    {
        use fdlimit::raise_fd_limit;
        use tracing::{info, warn};

        match raise_fd_limit() {
            Ok(Outcome::LimitRaised { to, from: _ }) => info!("Raised fd limit to {}", to),
            Ok(Outcome::Unsupported) => {},
            Err(err) => warn!("Failed to raise fd limit due to {}, this may cause \"Too many files error\" when there are too many connections", err),
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
            .or_else(|_| load_config_from_env("HOME", "./.dandelion/config.rn"))
            .context(
                "Failed to load config from $SNAP_COMMON/config.rn or $HOME/.dandelion/config.rn",
            )?,
    };

    let engine = Engine::load_config("config", code).await?;

    engine.run().await
}

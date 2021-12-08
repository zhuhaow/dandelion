use specht2_core::{
    server::{Server, ServerConfig},
    Result,
};
use std::{fs::read_to_string, path::PathBuf};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "specht2", about = "CLI version of the Specht2 client")]
struct Opt {
    #[structopt(parse(from_os_str))]
    input: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    flexi_logger::Logger::try_with_env()
        .unwrap()
        .start()
        .unwrap();

    let opt: Opt = Opt::from_args();

    let config: ServerConfig = ron::de::from_str(&read_to_string(opt.input)?)?;
    Server::new(config).serve().await?;
    Ok(())
}

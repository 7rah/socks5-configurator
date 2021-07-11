use anyhow::Result;
use log::{error, info, LevelFilter};
use serde_derive::Deserialize;
use socks5_configurator::{
    common::{self, copy_tcp, Address},
    route::{Router, RouterConfig, Tag},
    socks5::{Socks5Listener, Socks5Stream},
};
use std::{
    fs::File,
    io::Read,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
};
use structopt::StructOpt;
use tokio::{
    io::{split, AsyncWriteExt},
    net::TcpStream,
    spawn,
};

#[derive(Debug, Deserialize)]
struct Config {
    route: RouterConfig,
    socks5: SocketAddr,
    listen: SocketAddr,
}

impl Config {
    fn from_file<P: AsRef<Path>>(path: P) -> Self {
        let mut f = File::open(path).unwrap();
        let mut toml_str = String::new();
        f.read_to_string(&mut toml_str).unwrap();
        let config: Config = toml::from_str(&toml_str).unwrap();
        config
    }
}

#[derive(Debug, StructOpt)]
#[structopt(
    name = "socks5-configurator",
    about = "An rust implementation of socks5-configurator"
)]
struct Opt {
    #[structopt(
        parse(from_os_str),
        short = "c",
        long = "config",
        help = "toml config file path"
    )]
    config_file: PathBuf,
}

#[tokio::main]
async fn main() {
    let opt = Opt::from_args();
    let _ = env_logger::builder()
        .filter_level(LevelFilter::Info)
        .try_init();
    let config = Config::from_file(opt.config_file);
    let listener = Socks5Listener::bind(&config.listen).await.unwrap();
    let router = Arc::new(Router::init(&config.route).unwrap());
    let socks5 = Arc::new(config.socks5);
    info!("starting serve");

    loop {
        let router = router.clone();
        let socks5 = socks5.clone();
        match listener.accept().await {
            Ok((stream, addr)) => {
                spawn(async move {
                    match serve(router, socks5, stream, &addr).await {
                        Ok(()) => {}
                        Err(e) => error!("{}", e),
                    }
                });
            }
            Err(e) => error!("{}", e),
        }
    }
}

#[inline]
async fn serve(
    router: Arc<Router>,
    socks5: Arc<SocketAddr>,
    inbound: TcpStream,
    addr: &Address,
) -> Result<()> {
    let outbound = match router.match_tag(addr) {
        Tag::Proxy => {
            info!("PROXY: {}", addr);
            Socks5Stream::connect(&socks5, addr).await?
        }
        Tag::Direct => {
            info!("DIRECT: {}", addr);
            match addr {
                Address::SocketAddr(addr) => TcpStream::connect(addr).await?,
                Address::Domain(domain, port) => TcpStream::connect((&domain[..], *port)).await?,
            }
        }
    };

    let (mut ri, mut wi) = split(inbound);
    let (mut ro, mut wo) = split(outbound);
    let c1 = common::copy_tcp(&mut ri, &mut wo);
    let c2 = copy_tcp(&mut ro, &mut wi);

    let e = tokio::select! {
        e = c1 => {e}
        e = c2 => {e}
    };
    e?;

    let mut inbound = ri.unsplit(wi);
    let mut outbound = ro.unsplit(wo);
    let _ = inbound.shutdown().await;
    let _ = outbound.shutdown().await;

    Ok(())
}

use std::{env, error::Error};

use tokio::net::TcpListener;
use topcoat::asset::AssetBundle;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let router = accountability_app::router(AssetBundle::load()?).build();
    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_owned());
    let port = env::var("PORT")
        .unwrap_or_else(|_| "3002".to_owned())
        .parse::<u16>()?;
    let listener = TcpListener::bind((host.as_str(), port)).await?;
    println!(
        "Topcoat hybrid example: http://{}/hybrid",
        listener.local_addr()?
    );
    topcoat::serve(listener, router).await?;
    Ok(())
}

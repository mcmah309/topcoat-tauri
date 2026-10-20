#![forbid(unsafe_code)]
//! Hosted Topcoat pages with local Rust procedures in Tauri.

mod hosted_ui;
mod transport;

pub use transport::{
    Builder as TransportBuilder, DEFAULT_SCHEME, Error as TransportError, Origin, OriginError,
    Platform, Session,
};

use reqwest::{Client, Url};
use std::time::Duration;
use tauri::{Runtime, WebviewUrl, plugin::TauriPlugin};
use topcoat::router::RouterBuilder;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid hosted page URL: {0}")]
    Url(String),
    #[error(
        "hosted page must use HTTPS (loopback HTTP is allowed), without credentials or a fragment"
    )]
    UntrustedHost,
    #[error(transparent)]
    Client(#[from] reqwest::Error),
    #[error(transparent)]
    Protocol(#[from] TransportError),
}

/// A hosted page and its Topcoat assets, backed by a local Rust procedure router.
///
/// The entry path is taken from the supplied URL, so this has no knowledge of
/// application-specific routes or APIs. Only that page and `/_topcoat/assets/`
/// are fetched remotely. All other requests reach your local router.
#[derive(Clone)]
pub struct HostedApp {
    page: Url,
    client: Client,
}

impl HostedApp {
    /// Select a trusted application host. Requests time out after ten seconds
    /// and do not follow redirects. The remote document can call local procedures,
    /// so only use a host whose application code you trust.
    pub fn new(page: &str) -> Result<Self, Error> {
        let page = Url::parse(page).map_err(|error| Error::Url(error.to_string()))?;
        let loopback = matches!(page.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        if !(page.scheme() == "https" && page.has_host() || page.scheme() == "http" && loopback)
            || !page.username().is_empty()
            || page.password().is_some()
            || page.fragment().is_some()
        {
            return Err(Error::UntrustedHost);
        }
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self { page, client })
    }

    pub fn page_url(&self) -> &Url {
        &self.page
    }

    /// Reuse the Rust HTTP client for your app's remote service implementation.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// The local protocol URL corresponding to the remote entry page.
    pub fn webview_url(&self) -> WebviewUrl {
        let mut local = Url::parse("topcoat://localhost/").expect("constant protocol URL");
        local.set_path(self.page.path());
        local.set_query(self.page.query());
        WebviewUrl::CustomProtocol(local)
    }

    /// Add hosted page/asset delivery to your existing local procedure router.
    /// This also allows headless tests through [`TransportBuilder::session`].
    pub fn router(&self, procedures: RouterBuilder) -> RouterBuilder {
        procedures.layer(hosted_ui::HostedUi {
            client: self.client.clone(),
            page: self.page.clone(),
        })
    }

    /// Build the Tauri plugin. Install it before creating the application window.
    pub fn plugin<R: Runtime>(&self, procedures: RouterBuilder) -> Result<TauriPlugin<R>, Error> {
        Ok(TransportBuilder::new(self.router(procedures)).build()?)
    }
}

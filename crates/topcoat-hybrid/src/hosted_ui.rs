use reqwest::{Client, Url};
use topcoat::{
    context::Cx,
    router::{
        Body, Layer, LayerFuture, Next, Path, error::internal_server_error, request,
        response::Response,
    },
};

pub(crate) struct HostedUi {
    pub client: Client,
    pub page: Url,
}

impl Layer for HostedUi {
    fn path(&self) -> Option<&Path> {
        None
    }

    fn handle<'a>(&'a self, cx: &'a Cx, body: Body, next: Next<'a>) -> LayerFuture<'a> {
        Box::pin(async move {
            let uri = request::uri(cx);
            let path = uri.path();
            let method = request::method(cx);
            if !matches!(method.as_str(), "GET" | "HEAD")
                || !(path == self.page.path() || path.starts_with("/_topcoat/assets/"))
            {
                return next.run(cx, body).await;
            }
            let url = if path == self.page.path() {
                self.page.clone()
            } else {
                self.page
                    .join(uri.path_and_query().map_or(path, |value| value.as_str()))
                    .map_err(internal_server_error)?
            };
            let upstream = self
                .client
                .request(method.clone(), url)
                .send()
                .await
                .map_err(internal_server_error)?;
            if upstream.status().is_redirection() {
                return Err(internal_server_error(std::io::Error::other(
                    "Hosted page or asset redirected",
                ))
                .into());
            }
            let upstream = upstream.error_for_status().map_err(internal_server_error)?;
            let content_type = upstream.headers().get("content-type").cloned();
            let bytes = upstream.bytes().await.map_err(internal_server_error)?;
            let mut response = Response::builder().header("cache-control", "no-store");
            if let Some(value) = content_type {
                response = response.header("content-type", value);
            }
            Ok(response
                .body(Body::from(bytes.to_vec()))
                .map_err(internal_server_error)?)
        })
    }
}

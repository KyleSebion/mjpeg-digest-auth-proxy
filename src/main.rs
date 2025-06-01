use axum::{
    Router,
    body::Body,
    extract::State,
    http::{Response, StatusCode},
    response::IntoResponse,
    routing::get,
};
use clap::Parser;
use diqwest::WithDigestAuth;
use futures::{FutureExt, TryStreamExt};
use reqwest::{Client, header};
use std::sync::Arc;
use tokio::net::TcpListener;

#[derive(Parser)]
#[command(version)]
struct Opt {
    #[clap(short, long, default_value = "127.0.0.1:11111")]
    binding: String,
    /// upstream mjpeg url
    url: String,
    /// upstream mjpeg server username
    #[clap(short, long, default_value = "username")]
    username: String,
    /// upstream mjpeg server password
    #[clap(short, long, default_value = "password")]
    password: String,
}
struct AppState {
    client: Client,
    opt: Opt,
}
impl AppState {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            opt: Opt::parse(),
            client: Client::new(),
        })
    }
}
#[tokio::main]
async fn main() {
    let state = AppState::new();
    let app = Router::new()
        .route("/", get(mjpeg))
        .with_state(state.clone());
    let listener = TcpListener::bind(&state.opt.binding)
        .await
        .expect("bind failed");
    axum::serve(listener, app)
        .with_graceful_shutdown(tokio::signal::ctrl_c().map(|_| ()))
        .await
        .expect("serve failed");
}
async fn mjpeg(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let err_bg = StatusCode::BAD_GATEWAY.into_response();
    let ures = match state
        .client
        .get(&state.opt.url)
        .send_with_digest_auth(&state.opt.username, &state.opt.password)
        .await
    {
        Ok(ures) => ures,
        Err(err) => {
            eprintln!("{err:?}");
            return err_bg;
        }
    };
    if ures.status() != StatusCode::OK {
        return err_bg;
    }
    let content_type = match ures.headers().get(header::CONTENT_TYPE) {
        Some(content_type) => content_type.clone(),
        None => {
            eprintln!("{} missing", header::CONTENT_TYPE);
            return err_bg;
        }
    };
    let stream = ures.bytes_stream().map_err(std::io::Error::other);
    Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from_stream(stream))
        .expect("error building response")
}

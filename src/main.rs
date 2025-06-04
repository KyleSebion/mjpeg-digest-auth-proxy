use axum::{
    Extension, Router,
    body::Body,
    extract::{ConnectInfo, State, connect_info::IntoMakeServiceWithConnectInfo},
    http::{Request, Response, StatusCode},
    response::IntoResponse,
    routing::get,
};
use clap::Parser;
use diqwest::WithDigestAuth;
use futures::{FutureExt, TryStreamExt};
use reqwest::{Client, ClientBuilder, header};
use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing_appender::rolling;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(version)]
struct Opt {
    #[clap(short, long, default_value = "127.0.0.1:11111")]
    binding: String,

    /// upstream mjpeg url
    url: String,
    /// upstream mjpeg server username
    #[clap(short, long, env = "MDAP_USERNAME", default_value = "username")]
    username: String,
    /// upstream mjpeg server password
    #[clap(short, long, env = "MDAP_PASSWORD", default_value = "password")]
    password: String,

    /// allow insecure upstream server connections
    #[clap(short, long)]
    insecure: bool,

    /// enable logging to daily file. supply a value to override the default log directory [default: logs]
    #[clap(short, long, num_args=0..=1, require_equals=true, default_missing_value = "logs")]
    log_dir: Option<String>,
}
struct AppState {
    client: Client,
    opt: Opt,
}
impl AppState {
    fn new() -> Arc<Self> {
        let opt = Opt::parse();
        let client = ClientBuilder::new()
            .danger_accept_invalid_certs(opt.insecure)
            .build()
            .expect("failed to build client");
        Arc::new(Self { opt, client })
    }
}
#[derive(Clone)]
struct RqId(Arc<AtomicU64>);
impl RqId {
    fn new() -> Self {
        Self(Arc::new(AtomicU64::new(1)))
    }
    fn extension() -> Extension<Self> {
        Extension(Self::new())
    }
    fn next(&self) -> u64 {
        self.0.fetch_add(1, Ordering::Relaxed)
    }
}
fn setup_tracing(state: Arc<AppState>) {
    let sub = tracing_subscriber::fmt().with_env_filter(
        EnvFilter::try_from_default_env()
            .or_else(|_| {
                EnvFilter::try_new(format!(
                    "{}=debug,tower_http=debug,axum::rejection=trace",
                    env!("CARGO_CRATE_NAME")
                ))
            })
            .expect("tracing setup failed"),
    );
    if let Some(dir) = &state.opt.log_dir {
        let file = rolling::daily(dir, "");
        sub.with_writer(file).with_ansi(false).init();
    } else {
        sub.init();
    }
}
fn mk_app(state: Arc<AppState>) -> IntoMakeServiceWithConnectInfo<Router, SocketAddr> {
    Router::new()
        .route("/", get(mjpeg))
        .with_state(state)
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &Request<_>| {
                let ext = request.extensions();
                let client_addr = ext
                    .get::<ConnectInfo<SocketAddr>>()
                    .map_or_else(|| "unknown".into(), |a| a.0.to_string());
                let id = ext.get::<RqId>().map_or_else(|| 0, |id| id.next());
                tracing::info_span!(
                    "request",
                    id = %id,
                    client = %client_addr,
                    method = %request.method(),
                    uri = %request.uri(),
                )
            }),
        )
        .layer(RqId::extension())
        .into_make_service_with_connect_info::<SocketAddr>()
}
async fn mk_listener(state: Arc<AppState>) -> TcpListener {
    TcpListener::bind(&state.opt.binding)
        .await
        .expect("bind failed")
}
#[tokio::main]
async fn main() {
    let state = AppState::new();
    setup_tracing(state.clone());
    let app = mk_app(state.clone());
    let listener = mk_listener(state.clone()).await;
    tracing::debug!(
        "listening on {}; proxying to {}",
        listener.local_addr().expect("local_addr"),
        &state.opt.url
    );
    axum::serve(listener, app)
        .with_graceful_shutdown(tokio::signal::ctrl_c().map(|_| ()))
        .await
        .expect("serve failed");
    tracing::debug!("end");
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
            tracing::error!("{err:?}");
            return err_bg;
        }
    };
    if ures.status() != StatusCode::OK {
        return err_bg;
    }
    let content_type = match ures.headers().get(header::CONTENT_TYPE) {
        Some(content_type) => content_type.clone(),
        None => {
            tracing::error!("{} missing", header::CONTENT_TYPE);
            return err_bg;
        }
    };
    let stream = ures.bytes_stream().map_err(std::io::Error::other);
    Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from_stream(stream))
        .expect("error building response")
}

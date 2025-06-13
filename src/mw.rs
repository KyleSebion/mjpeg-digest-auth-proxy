use axum::{
    Router,
    body::{Body, Bytes},
    extract::Request,
    http::Response as HttpResponse,
    middleware::{Next, from_fn},
    response::Response,
};
use futures::Stream;
use std::{
    marker::Unpin,
    pin::Pin,
    task::{Context, Poll},
};
use tracing::Span;
struct StreamWithLoggedEnd<S> {
    inner: S,
    span: Span,
}
impl<S> StreamWithLoggedEnd<S> {
    fn new(inner: S, span: Span) -> Self {
        Self { inner, span }
    }
}
impl<S, E> Stream for StreamWithLoggedEnd<S>
where
    S: Stream<Item = Result<Bytes, E>> + Unpin,
{
    type Item = Result<Bytes, E>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }
}
impl<S> Drop for StreamWithLoggedEnd<S> {
    fn drop(&mut self) {
        let _s = self.span.enter();
        tracing::debug!("stream ended");
    }
}
async fn logged_end_middleware_fn(rq: Request, next: Next) -> Response {
    let (parts, body) = next.run(rq).await.into_parts();
    let stream = StreamWithLoggedEnd::new(body.into_data_stream(), Span::current());
    HttpResponse::from_parts(parts, Body::from_stream(stream))
}
pub trait LayerTraceResponseEnd {
    fn layer_trace_response_end(self) -> Self;
}
impl LayerTraceResponseEnd for Router {
    fn layer_trace_response_end(self) -> Self {
        self.layer(from_fn(logged_end_middleware_fn))
    }
}

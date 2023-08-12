use std::net::{Ipv4Addr, SocketAddr};

use axum::{routing, Router, Server};

use hyper::Error;
use tower_http::{trace::{TraceLayer, DefaultOnRequest, DefaultMakeSpan, DefaultOnResponse}, LatencyUnit};
use tracing::Level;
use crate::relayer;
use tokio::sync::mpsc;
use utoipa::{
    openapi::security::{ApiKey, ApiKeyValue, SecurityScheme},
    Modify, OpenApi,
};

use utoipa_rapidoc::RapiDoc;
use utoipa_redoc::{Redoc, Servable};
use utoipa_swagger_ui::SwaggerUi;

pub async fn start(rx: mpsc::UnboundedSender<(String, String)>) -> Result<(), Error> {
    #[derive(OpenApi)]
    #[openapi(
        paths(
            relayer::send,
        ),
        components(
            schemas(relayer::Message)
        ),
        modifiers(&SecurityAddon),
        tags(
            (name = "signal", description = "Signal API")
        )
    )]
    struct ApiDoc;

    struct SecurityAddon;

    impl Modify for SecurityAddon {
        fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
            if let Some(components) = openapi.components.as_mut() {
                components.add_security_scheme(
                    "api_key",
                    SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("signal_apikey"))),
                )
            }
        }
    }

    let app = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .merge(Redoc::with_url("/redoc", ApiDoc::openapi()))
        // There is no need to create `RapiDoc::with_openapi` because the OpenApi is served
        // via SwaggerUi instead we only make rapidoc to point to the existing doc.
        .merge(RapiDoc::new("/api-docs/openapi.json").path("/rapidoc"))
        // Alternative to above
        // .merge(RapiDoc::with_openapi("/api-docs/openapi2.json", ApiDoc::openapi()).path("/rapidoc"))
        .route(
            "/message/:destination",
            routing::post(relayer::send),
        )
        .with_state(rx)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(
                    DefaultMakeSpan::new().include_headers(true)
                )
                .on_request(
                    DefaultOnRequest::new().level(Level::INFO)
                )
                .on_response(
                    DefaultOnResponse::new()
                        .level(Level::INFO)
                        .latency_unit(LatencyUnit::Micros)
                )
                // on so on for `on_eos`, `on_body_chunk`, and `on_failure`
        );

    let address = SocketAddr::from((Ipv4Addr::UNSPECIFIED, 8080));
    Server::bind(&address).serve(app.into_make_service()).await
}
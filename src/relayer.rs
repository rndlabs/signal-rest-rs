use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use hyper::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;
use utoipa::ToSchema;

/// A message to send.
#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct Message {
    content: String,
}

/// Send a message to a destination.
#[utoipa::path(
    post,
    path = "/message/{destination}",
    request_body = Message,
    responses(
        (status = 200, description = "Message sent successfully"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("destination" = String, Path, description = "The UUID of the destination")
    ),
    security(
        (), // <-- make optional authentication
        ("api_key" = [])
    )
)]
pub async fn send(
    Path(destination): Path<String>,
    State(session): State<UnboundedSender<(String, String)>>,
    _headers: HeaderMap,
    Json(message): Json<Message>,
) -> impl IntoResponse {

    println!("received message: {}", message.content);

    match session.send((destination, message.content)) {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
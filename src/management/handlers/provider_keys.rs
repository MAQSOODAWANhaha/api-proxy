
use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde::{Deserialize, Serialize};

use entity::user_provider_keys;
use crate::management::server::AppState;

#[derive(Serialize, Deserialize)]
pub struct ProviderKeyPayload {
    pub name: String,
    pub provider: String,
    pub api_key: String,
    pub weight: i32,
    pub is_active: bool,
}

pub async fn list_provider_keys(State(state): State<AppState>) -> impl IntoResponse {
    let keys = user_provider_keys::Entity::find()
        .all(&*state.database)
        .await
        .unwrap();
    (StatusCode::OK, Json(keys))
}

pub async fn create_provider_key(
    State(state): State<AppState>,
    Json(payload): Json<ProviderKeyPayload>,
) -> impl IntoResponse {
    let new_key = user_provider_keys::ActiveModel {
        name: Set(payload.name),
        provider_type_id: Set(1), // Simplified for now
        api_key: Set(payload.api_key),
        is_active: Set(payload.is_active),
        user_id: Set(1), // Simplified for now
        weight: Set(Some(payload.weight)),
        ..Default::default()
    };

    match new_key.insert(&*state.database).await {
        Ok(key) => (StatusCode::CREATED, Json(serde_json::to_value(key).unwrap())).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn update_provider_key(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(payload): Json<ProviderKeyPayload>,
) -> impl IntoResponse {
    if let Ok(Some(key)) = user_provider_keys::Entity::find_by_id(id)
        .one(&*state.database)
        .await
    {
        let mut key: user_provider_keys::ActiveModel = key.into();
        key.name = Set(payload.name);
        key.api_key = Set(payload.api_key);
        key.is_active = Set(payload.is_active);
        key.weight = Set(Some(payload.weight));
        
        match key.update(&*state.database).await {
            Ok(updated_key) => (StatusCode::OK, Json(serde_json::to_value(updated_key).unwrap())).into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response(),
        }
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Key not found" })),
        )
            .into_response()
    }
}

pub async fn delete_provider_key(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    match user_provider_keys::Entity::delete_by_id(id)
        .exec(&*state.database)
        .await
    {
        Ok(res) => {
            if res.rows_affected == 1 {
                StatusCode::NO_CONTENT
            } else {
                StatusCode::NOT_FOUND
            }
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

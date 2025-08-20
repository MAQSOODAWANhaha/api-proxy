
use crate::management::handlers::auth_utils::extract_user_id_from_headers;
use crate::management::{response, server::AppState};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use chrono::{DateTime, NaiveDate, Utc};
use jsonwebtoken::{DecodingKey, Validation, decode};
use sea_orm::QueryOrder; // for order_by()
use sea_orm::prelude::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

/// 获取用户提供商密钥列表
pub async fn get_user_provider_keys(
    State(state): State<AppState>,
    Query(query): Query<UserProviderKeyQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    use entity::provider_types::Entity as ProviderType;
    use entity::user_provider_keys::{self, Entity as UserProviderKey};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let db = state.database.as_ref();

    // 从JWT token中提取用户ID
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response.into_response(),
    };

    // 构建查询条件
    let mut select = UserProviderKey::find().filter(user_provider_keys::Column::UserId.eq(user_id));

    // 应用服务商类型筛选
    if let Some(provider_type_id) = query.provider_type_id {
        select = select.filter(user_provider_keys::Column::ProviderTypeId.eq(provider_type_id));
    }

    // 应用启用状态筛选（默认只返回启用的）
    let is_active = query.is_active.unwrap_or(true);
    select = select.filter(user_provider_keys::Column::IsActive.eq(is_active));

    // 执行查询并关联 provider_types 表
    let provider_keys = match select.find_also_related(ProviderType).all(db).await {
        Ok(data) => data,
        Err(err) => {
            tracing::error!("Failed to fetch user provider keys: {}", err);
            return response::error::<serde_json::Value>(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch user provider keys",
            ).into_response();
        }
    };

    // 构建简化的响应数据（仅用于下拉选择）
    let mut user_provider_keys = Vec::new();

    for (provider_key, _provider_type) in provider_keys {
        // 极简化的响应结构
        let response_key = json!({
            "id": provider_key.id,
            "name": provider_key.name,
            "display_name": provider_key.name // 使用name作为display_name
        });

        user_provider_keys.push(response_key);
    }

    let data = json!({
        "user_provider_keys": user_provider_keys
    });

    response::success(data).into_response()
}

/// 用户提供商密钥查询参数
#[derive(Debug, Deserialize)]
pub struct UserProviderKeyQuery {
    /// 服务商类型ID筛选
    pub provider_type_id: Option<i32>,
    /// 是否启用筛选
    pub is_active: Option<bool>,
}

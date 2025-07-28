//! # 认证管理处理器

use crate::management::server::AppState;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, HeaderMap};
use axum::response::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sea_orm::{entity::*, query::*};
use entity::{
    user_service_apis,
    user_service_apis::Entity as UserServiceApis,
    users::Entity as Users,
    provider_types,
    provider_types::Entity as ProviderTypes,
};
use chrono::{Utc, Duration};
use jsonwebtoken::{encode, decode, Header, EncodingKey, DecodingKey, Validation};
use rand;

/// 登录请求
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    /// 用户名
    pub username: String,
    /// 密码  
    pub password: String,
}

/// 登录响应
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    /// JWT token
    pub token: String,
    /// 用户信息
    pub user: UserInfo,
}

/// 用户信息
#[derive(Debug, Serialize)]
pub struct UserInfo {
    /// 用户ID
    pub id: i32,
    /// 用户名
    pub username: String,
    /// 邮箱
    pub email: String,
    /// 是否为管理员
    pub is_admin: bool,
}

/// JWT Claims
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// 用户ID
    pub sub: String,
    /// 用户名
    pub username: String,
    /// 是否为管理员
    pub is_admin: bool,
    /// 过期时间
    pub exp: usize,
    /// 签发时间
    pub iat: usize,
}

/// 用户登录（临时简化版本，暂时放开认证）
pub async fn login(
    State(_state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    // 基本输入验证
    if request.username.is_empty() || request.password.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // 临时：直接返回成功响应，不做密码验证
    let now = Utc::now();
    let exp = now + Duration::hours(24);
    
    let claims = Claims {
        sub: "1".to_string(),
        username: request.username.clone(),
        is_admin: true, // 临时设为管理员
        exp: exp.timestamp() as usize,
        iat: now.timestamp() as usize,
    };

    // 从环境变量或配置获取JWT密钥
    let jwt_secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "change-me-in-production-jwt-secret-key".to_string());
    
    let token = match encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_ref()),
    ) {
        Ok(token) => token,
        Err(err) => {
            tracing::error!("JWT encoding error: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    tracing::info!("User {} logged in successfully (simplified mode)", request.username);

    let response = LoginResponse {
        token,
        user: UserInfo {
            id: 1,
            username: request.username,
            email: "admin@example.com".to_string(),
            is_admin: true,
        },
    };

    Ok(Json(response))
}

/// 验证token响应
#[derive(Debug, Serialize)]
pub struct ValidateTokenResponse {
    /// token是否有效
    pub valid: bool,
    /// 用户信息（如果token有效）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<UserInfo>,
}

/// 验证JWT Token
pub async fn validate_token(
    State(_state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ValidateTokenResponse>, StatusCode> {
    // 从Authorization头中提取token
    let auth_header = match headers.get("Authorization") {
        Some(header) => match header.to_str() {
            Ok(header_str) => header_str,
            Err(_) => return Ok(Json(ValidateTokenResponse { valid: false, user: None })),
        },
        None => return Ok(Json(ValidateTokenResponse { valid: false, user: None })),
    };

    // 检查Bearer前缀
    if !auth_header.starts_with("Bearer ") {
        return Ok(Json(ValidateTokenResponse { valid: false, user: None }));
    }

    let token = &auth_header[7..]; // 移除"Bearer "前缀

    // 从环境变量或配置获取JWT密钥
    let jwt_secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "change-me-in-production-jwt-secret-key".to_string());
    
    // 验证JWT token
    let validation = Validation::default();
    let token_data = match decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_ref()),
        &validation,
    ) {
        Ok(data) => data,
        Err(err) => {
            tracing::debug!("Token validation failed: {}", err);
            return Ok(Json(ValidateTokenResponse { valid: false, user: None }));
        }
    };

    // 检查token是否过期
    let now = Utc::now().timestamp() as usize;
    if token_data.claims.exp < now {
        return Ok(Json(ValidateTokenResponse { valid: false, user: None }));
    }

    // 构造用户信息
    let user_info = UserInfo {
        id: token_data.claims.sub.parse().unwrap_or(1),
        username: token_data.claims.username,
        email: "admin@example.com".to_string(), // 临时固定值
        is_admin: token_data.claims.is_admin,
    };

    Ok(Json(ValidateTokenResponse {
        valid: true,
        user: Some(user_info),
    }))
}

/// API密钥查询参数
#[derive(Debug, Deserialize)]
pub struct ApiKeyQuery {
    /// 页码
    pub page: Option<u32>,
    /// 每页大小
    pub limit: Option<u32>,
    /// 用户ID过滤
    pub user_id: Option<i32>,
    /// 状态过滤
    pub status: Option<String>,
}

/// 创建API密钥请求
#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    /// 用户ID
    pub user_id: i32,
    /// 服务提供商类型ID（1=OpenAI, 2=Gemini, 3=Claude）
    pub provider_type_id: i32,
    /// 密钥名称
    pub name: String,
    /// 描述
    pub description: Option<String>,
    /// 过期时间（天数）
    pub expires_in_days: Option<u32>,
    /// 权限范围
    pub scopes: Option<Vec<String>>,
}

/// API密钥响应
#[derive(Debug, Serialize)]
pub struct ApiKeyResponse {
    /// 密钥ID
    pub id: i32,
    /// 密钥名称
    pub name: String,
    /// 密钥（仅在创建时返回）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// 密钥前缀（用于显示）
    pub key_prefix: String,
    /// 用户ID
    pub user_id: i32,
    /// 描述
    pub description: Option<String>,
    /// 状态
    pub status: String,
    /// 权限范围
    pub scopes: Vec<String>,
    /// 使用次数
    pub usage_count: u64,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 过期时间
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    /// 最后使用时间
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// 列出API密钥
pub async fn list_api_keys(
    State(state): State<AppState>,
    Query(query): Query<ApiKeyQuery>,
) -> Result<Json<Value>, StatusCode> {
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);
    let offset = (page - 1) * limit;
    
    // 构建查询条件
    let mut select = UserServiceApis::find();
    
    // 用户ID过滤
    if let Some(user_id) = query.user_id {
        select = select.filter(user_service_apis::Column::UserId.eq(user_id));
    }
    
    // 状态过滤
    if let Some(status) = &query.status {
        match status.as_str() {
            "active" => select = select.filter(user_service_apis::Column::IsActive.eq(true)),
            "inactive" => select = select.filter(user_service_apis::Column::IsActive.eq(false)),
            _ => {}
        }
    }
    
    // 分页查询
    let api_keys_result = select
        .offset(offset as u64)
        .limit(limit as u64)
        .order_by_desc(user_service_apis::Column::CreatedAt)
        .find_also_related(ProviderTypes)
        .all(state.database.as_ref())
        .await;
        
    let api_keys_data = match api_keys_result {
        Ok(data) => data,
        Err(err) => {
            tracing::error!("Failed to fetch API keys: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    // 获取总数
    let mut count_select = UserServiceApis::find();
    if let Some(user_id) = query.user_id {
        count_select = count_select.filter(user_service_apis::Column::UserId.eq(user_id));
    }
    if let Some(status) = &query.status {
        match status.as_str() {
            "active" => count_select = count_select.filter(user_service_apis::Column::IsActive.eq(true)),
            "inactive" => count_select = count_select.filter(user_service_apis::Column::IsActive.eq(false)),
            _ => {}
        }
    }
    
    let total = match count_select.count(state.database.as_ref()).await {
        Ok(count) => count,
        Err(err) => {
            tracing::error!("Failed to count API keys: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    // 转换为响应格式
    let mut api_keys = Vec::new();
    for (api_key, provider_type) in api_keys_data {
        let provider = provider_type.unwrap_or(provider_types::Model {
            id: 0,
            name: "unknown".to_string(),
            display_name: "Unknown Provider".to_string(),
            base_url: "".to_string(),
            api_format: "".to_string(),
            default_model: None,
            max_tokens: None,
            rate_limit: None,
            timeout_seconds: None,
            health_check_path: None,
            auth_header_format: None,
            is_active: false,
            config_json: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        });
        let key_prefix = if api_key.api_key.len() > 15 {
            format!("{}...", &api_key.api_key[..15])
        } else {
            "sk-***...".to_string()
        };
        
        let scopes = get_api_key_scopes(&api_key).await; // 从数据库获取实际权限
        
        api_keys.push(ApiKeyResponse {
            id: api_key.id,
            name: api_key.name.unwrap_or_else(|| format!("{} API Key", provider.display_name)),
            key: None, // 永远不在列表中显示完整密钥
            key_prefix,
            user_id: api_key.user_id,
            description: api_key.description,
            status: if api_key.is_active { "active".to_string() } else { "inactive".to_string() },
            scopes,
            usage_count: api_key.total_requests.unwrap_or(0) as u64,
            created_at: api_key.created_at.and_utc(),
            expires_at: api_key.expires_at.map(|dt| dt.and_utc()),
            last_used_at: api_key.last_used.map(|dt| dt.and_utc()),
        });
    }

    let response = json!({
        "api_keys": api_keys,
        "pagination": {
            "page": page,
            "limit": limit,
            "total": total,
            "pages": ((total as f64) / (limit as f64)).ceil() as u32
        }
    });

    Ok(Json(response))
}

/// 获取服务提供商类型列表
pub async fn list_provider_types(
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    // 获取所有活跃的服务提供商类型
    let provider_types_result = ProviderTypes::find()
        .filter(provider_types::Column::IsActive.eq(true))
        .order_by_asc(provider_types::Column::Id)
        .all(state.database.as_ref())
        .await;
        
    let provider_types_data = match provider_types_result {
        Ok(data) => data,
        Err(err) => {
            tracing::error!("Failed to fetch provider types: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    // 转换为响应格式
    let provider_types: Vec<_> = provider_types_data
        .into_iter()
        .map(|provider| {
            json!({
                "id": provider.id,
                "name": provider.name,
                "display_name": provider.display_name,
                "base_url": provider.base_url,
                "api_format": provider.api_format,
                "default_model": provider.default_model,
                "is_active": provider.is_active
            })
        })
        .collect();

    let response = json!({
        "provider_types": provider_types
    });

    Ok(Json(response))
}

/// 从数据库获取API密钥的权限范围
async fn get_api_key_scopes(api_key: &user_service_apis::Model) -> Vec<String> {
    // 根据API密钥的配置解析权限范围
    let mut scopes = Vec::new();
    
    // 基础权限：所有API密钥都有的基本权限
    scopes.push("api:access".to_string());
    
    // 根据最大请求数判断权限等级
    if let Some(rate_limit) = api_key.rate_limit {
        if rate_limit >= 100 {
            scopes.push("api:high_rate".to_string());
        } else if rate_limit >= 50 {
            scopes.push("api:medium_rate".to_string());
        } else {
            scopes.push("api:low_rate".to_string());
        }
    } else {
        scopes.push("api:unlimited_rate".to_string());
    }
    
    // 根据最大令牌数判断权限范围
    if let Some(max_tokens) = api_key.max_tokens_per_day {
        if max_tokens >= 100000 {
            scopes.push("tokens:enterprise".to_string());
        } else if max_tokens >= 10000 {
            scopes.push("tokens:professional".to_string());
        } else {
            scopes.push("tokens:basic".to_string());
        }
    } else {
        scopes.push("tokens:unlimited".to_string());
    }
    
    // AI服务权限
    scopes.push("ai:chat".to_string());
    scopes.push("ai:completion".to_string());
    
    // 根据总请求数判断优先级权限
    if let Some(total_requests) = api_key.total_requests {
        if total_requests >= 1000 {
            scopes.push("priority:high".to_string());
            scopes.push("ai:advanced".to_string());
        } else if total_requests >= 100 {
            scopes.push("priority:medium".to_string());
        } else {
            scopes.push("priority:low".to_string());
        }
    } else {
        scopes.push("priority:new".to_string());
    }
    
    // 调度策略权限
    if api_key.scheduling_strategy.is_some() {
        scopes.push("scheduling:custom".to_string());
    } else {
        scopes.push("scheduling:default".to_string());
    }
    
    // 如果API密钥是活跃的，添加活跃权限
    if api_key.is_active {
        scopes.push("status:active".to_string());
    } else {
        scopes.push("status:inactive".to_string());
    }
    
    scopes
}

/// 创建API密钥
pub async fn create_api_key(
    State(state): State<AppState>,
    Json(request): Json<CreateApiKeyRequest>,
) -> Result<Json<Value>, StatusCode> {
    // 验证输入
    if request.name.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    if request.user_id <= 0 || request.provider_type_id <= 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // 验证用户是否存在
    let user_exists = match Users::find_by_id(request.user_id).one(state.database.as_ref()).await {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(err) => {
            tracing::error!("Failed to check user existence: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    if !user_exists {
        return Ok(Json(json!({
            "success": false,
            "message": "User not found"
        })));
    }

    // 验证服务提供商类型是否存在
    let provider_type = match ProviderTypes::find_by_id(request.provider_type_id).one(state.database.as_ref()).await {
        Ok(Some(provider)) => provider,
        Ok(None) => {
            return Ok(Json(json!({
                "success": false,
                "message": "Invalid provider type"
            })));
        },
        Err(err) => {
            tracing::error!("Failed to check provider type existence: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // 检查该用户是否已经为该服务提供商创建过API密钥
    let existing_api = match UserServiceApis::find()
        .filter(user_service_apis::Column::UserId.eq(request.user_id))
        .filter(user_service_apis::Column::ProviderTypeId.eq(request.provider_type_id))
        .one(state.database.as_ref())
        .await
    {
        Ok(api) => api,
        Err(err) => {
            tracing::error!("Failed to check existing API key: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    if existing_api.is_some() {
        return Ok(Json(json!({
            "success": false,
            "message": format!("API key for {} already exists. Each user can only have one API key per provider type.", provider_type.display_name)
        })));
    }

    // 生成新的API密钥
    let api_key = format!("sk-proj-{}", generate_random_key(32));
    let api_secret = generate_random_key(64);
    
    let expires_at = request.expires_in_days
        .map(|days| Utc::now().naive_utc() + Duration::days(days as i64));

    let now = Utc::now().naive_utc();

    // 创建API密钥记录
    let new_api_key = user_service_apis::ActiveModel {
        user_id: Set(request.user_id),
        provider_type_id: Set(request.provider_type_id),
        api_key: Set(api_key.clone()),
        api_secret: Set(api_secret),
        name: Set(Some(request.name.clone())),
        description: Set(request.description.clone()),
        scheduling_strategy: Set(Some("round_robin".to_string())),
        retry_count: Set(Some(3)),
        timeout_seconds: Set(Some(30)),
        rate_limit: Set(Some(100)),
        max_tokens_per_day: Set(Some(10000)),
        used_tokens_today: Set(Some(0)),
        total_requests: Set(Some(0)),
        successful_requests: Set(Some(0)),
        last_used: Set(None),
        expires_at: Set(expires_at),
        is_active: Set(true),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };

    let insert_result = UserServiceApis::insert(new_api_key).exec(state.database.as_ref()).await;
    
    let api_key_id = match insert_result {
        Ok(result) => result.last_insert_id,
        Err(err) => {
            tracing::error!("Failed to create API key: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let api_key_response = ApiKeyResponse {
        id: api_key_id,
        name: request.name,
        key: Some(api_key.clone()), // 仅在创建时返回完整密钥
        key_prefix: format!("{}...", &api_key[..15]),
        user_id: request.user_id,
        description: request.description,
        status: "active".to_string(),
        scopes: request.scopes.unwrap_or_else(|| vec!["chat:read".to_string(), "chat:write".to_string()]),
        usage_count: 0,
        created_at: Utc::now(),
        expires_at: expires_at.map(|dt| dt.and_utc()),
        last_used_at: None,
    };

    let response = json!({
        "success": true,
        "api_key": api_key_response,
        "message": "API key created successfully. Please save it now as it won't be shown again."
    });

    Ok(Json(response))
}

/// 获取单个API密钥
pub async fn get_api_key(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
) -> Result<Json<Value>, StatusCode> {
    if key_id <= 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // 从数据库获取API密钥
    let api_key_result = UserServiceApis::find_by_id(key_id)
        .find_also_related(ProviderTypes)
        .one(state.database.as_ref())
        .await;

    let (api_key, provider_type) = match api_key_result {
        Ok(Some(data)) => data,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(err) => {
            tracing::error!("Failed to fetch API key {}: {}", key_id, err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let provider = provider_type.unwrap_or(provider_types::Model {
        id: 0,
        name: "unknown".to_string(),
        display_name: "Unknown Provider".to_string(),
        base_url: "".to_string(),
        api_format: "".to_string(),
        default_model: None,
        max_tokens: None,
        rate_limit: None,
        timeout_seconds: None,
        health_check_path: None,
        auth_header_format: None,
        is_active: false,
        config_json: None,
        created_at: chrono::Utc::now().naive_utc(),
        updated_at: chrono::Utc::now().naive_utc(),
    });

    let key_prefix = if api_key.api_key.len() > 15 {
        format!("{}...", &api_key.api_key[..15])
    } else {
        "sk-***...".to_string()
    };

    let api_key_response = ApiKeyResponse {
        id: api_key.id,
        name: api_key.name.unwrap_or_else(|| format!("{} API Key", provider.display_name)),
        key: None, // 永远不返回完整密钥
        key_prefix,
        user_id: api_key.user_id,
        description: api_key.description,
        status: if api_key.is_active { "active".to_string() } else { "inactive".to_string() },
        scopes: vec!["chat:read".to_string(), "chat:write".to_string()], // TODO: 从数据库获取实际权限
        usage_count: api_key.total_requests.unwrap_or(0) as u64,
        created_at: api_key.created_at.and_utc(),
        expires_at: api_key.expires_at.map(|dt| dt.and_utc()),
        last_used_at: api_key.last_used.map(|dt| dt.and_utc()),
    };

    Ok(Json(serde_json::to_value(api_key_response).unwrap()))
}

/// 更新API密钥请求
#[derive(Debug, Deserialize)]
pub struct UpdateApiKeyRequest {
    /// 密钥名称
    pub name: Option<String>,
    /// 描述
    pub description: Option<String>,
    /// 是否激活
    pub is_active: Option<bool>,
}

/// 更新API密钥
pub async fn update_api_key(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
    Json(request): Json<UpdateApiKeyRequest>,
) -> Result<Json<Value>, StatusCode> {
    if key_id <= 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // 检查API密钥是否存在
    let existing_key = match UserServiceApis::find_by_id(key_id).one(state.database.as_ref()).await {
        Ok(Some(key)) => key,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(err) => {
            tracing::error!("Failed to check API key existence: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // 更新API密钥信息
    let now = Utc::now().naive_utc();
    let mut api_key: user_service_apis::ActiveModel = existing_key.into();
    
    if let Some(name) = request.name {
        if !name.is_empty() {
            api_key.name = Set(Some(name));
        }
    }
    
    if let Some(description) = request.description {
        api_key.description = Set(Some(description));
    }
    
    if let Some(is_active) = request.is_active {
        api_key.is_active = Set(is_active);
    }
    
    api_key.updated_at = Set(now);

    match api_key.update(state.database.as_ref()).await {
        Ok(_) => {
            let response = json!({
                "success": true,
                "message": format!("API key {} has been updated", key_id)
            });
            Ok(Json(response))
        }
        Err(err) => {
            tracing::error!("Failed to update API key {}: {}", key_id, err);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// 撤销API密钥
pub async fn revoke_api_key(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
) -> Result<Json<Value>, StatusCode> {
    if key_id <= 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // 检查API密钥是否存在
    let existing_key = match UserServiceApis::find_by_id(key_id).one(state.database.as_ref()).await {
        Ok(Some(key)) => key,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(err) => {
            tracing::error!("Failed to check API key existence: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // 更新API密钥状态为非活跃
    let now = Utc::now().naive_utc();
    let mut api_key: user_service_apis::ActiveModel = existing_key.into();
    api_key.is_active = Set(false);
    api_key.updated_at = Set(now);

    match api_key.update(state.database.as_ref()).await {
        Ok(_) => {
            let response = json!({
                "success": true,
                "message": format!("API key {} has been revoked", key_id),
                "revoked_at": now.and_utc()
            });
            Ok(Json(response))
        }
        Err(err) => {
            tracing::error!("Failed to revoke API key {}: {}", key_id, err);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// 生成随机密钥
fn generate_random_key(length: usize) -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    
    (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}
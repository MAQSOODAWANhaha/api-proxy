use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        insert_demo_request_statistics(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 删除演示数据
        manager
            .get_connection()
            .execute_unprepared("DELETE FROM request_statistics WHERE request_id LIKE 'demo-%'")
            .await?;
        Ok(())
    }
}

/// 插入演示请求统计数据
async fn insert_demo_request_statistics(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    // 直接执行SQL插入语句，避免类型问题
    let sql_statements = vec![
        // OpenAI API 请求 (user_service_api_id = 1)
        "INSERT INTO request_statistics (user_service_api_id, request_id, method, path, status_code, response_time_ms, request_size, response_size, tokens_prompt, tokens_completion, tokens_total, model_used, client_ip, user_agent, error_type, error_message, created_at) VALUES (1, 'demo-openai-1001', 'POST', '/v1/chat/completions', 200, 150, 1200, 3500, 100, 200, 300, 'gpt-4', '192.168.1.100', 'python-requests/2.31.0', NULL, NULL, '2025-07-24 23:15:30')",
        
        "INSERT INTO request_statistics (user_service_api_id, request_id, method, path, status_code, response_time_ms, request_size, response_size, tokens_prompt, tokens_completion, tokens_total, model_used, client_ip, user_agent, error_type, error_message, created_at) VALUES (1, 'demo-openai-1002', 'POST', '/v1/chat/completions', 200, 120, 800, 2800, 80, 180, 260, 'gpt-3.5-turbo', '10.0.0.50', 'curl/8.1.0', NULL, NULL, '2025-07-24 22:45:15')",
        
        "INSERT INTO request_statistics (user_service_api_id, request_id, method, path, status_code, response_time_ms, request_size, response_size, tokens_prompt, tokens_completion, tokens_total, model_used, client_ip, user_agent, error_type, error_message, created_at) VALUES (1, 'demo-openai-1003', 'GET', '/v1/models', 200, 50, 150, 500, NULL, NULL, NULL, NULL, '192.168.100.10', 'PostmanRuntime/7.32.3', NULL, NULL, '2025-07-24 21:30:45')",
        
        "INSERT INTO request_statistics (user_service_api_id, request_id, method, path, status_code, response_time_ms, request_size, response_size, tokens_prompt, tokens_completion, tokens_total, model_used, client_ip, user_agent, error_type, error_message, created_at) VALUES (1, 'demo-openai-1004', 'POST', '/v1/completions', 429, 30, 900, 150, NULL, NULL, NULL, 'text-davinci-003', '172.16.0.200', 'Mozilla/5.0', 'rate_limit_error', 'Rate limit exceeded', '2025-07-24 20:20:10')",
        
        "INSERT INTO request_statistics (user_service_api_id, request_id, method, path, status_code, response_time_ms, request_size, response_size, tokens_prompt, tokens_completion, tokens_total, model_used, client_ip, user_agent, error_type, error_message, created_at) VALUES (1, 'demo-openai-1005', 'POST', '/v1/embeddings', 200, 80, 600, 1200, NULL, NULL, NULL, 'text-embedding-ada-002', '192.168.1.100', 'python-requests/2.31.0', NULL, NULL, '2025-07-24 19:15:22')",
        
        // Gemini API 请求 (user_service_api_id = 2)
        "INSERT INTO request_statistics (user_service_api_id, request_id, method, path, status_code, response_time_ms, request_size, response_size, tokens_prompt, tokens_completion, tokens_total, model_used, client_ip, user_agent, error_type, error_message, created_at) VALUES (2, 'demo-gemini-2001', 'POST', '/v1/chat/completions', 200, 180, 1100, 3200, 120, 250, 370, 'gemini-pro', '10.0.0.50', 'curl/8.1.0', NULL, NULL, '2025-07-24 23:45:12')",
        
        "INSERT INTO request_statistics (user_service_api_id, request_id, method, path, status_code, response_time_ms, request_size, response_size, tokens_prompt, tokens_completion, tokens_total, model_used, client_ip, user_agent, error_type, error_message, created_at) VALUES (2, 'demo-gemini-2002', 'POST', '/v1/chat/completions', 200, 220, 1400, 4100, 150, 300, 450, 'gemini-1.5-pro', '192.168.100.10', 'python-requests/2.31.0', NULL, NULL, '2025-07-24 22:30:33')",
        
        "INSERT INTO request_statistics (user_service_api_id, request_id, method, path, status_code, response_time_ms, request_size, response_size, tokens_prompt, tokens_completion, tokens_total, model_used, client_ip, user_agent, error_type, error_message, created_at) VALUES (2, 'demo-gemini-2003', 'POST', '/v1/completions', 500, 60, 800, 100, NULL, NULL, NULL, 'gemini-pro', '172.16.0.200', 'PostmanRuntime/7.32.3', 'server_error', 'Internal server error', '2025-07-24 21:45:55')",
        
        "INSERT INTO request_statistics (user_service_api_id, request_id, method, path, status_code, response_time_ms, request_size, response_size, tokens_prompt, tokens_completion, tokens_total, model_used, client_ip, user_agent, error_type, error_message, created_at) VALUES (2, 'demo-gemini-2004', 'POST', '/v1/chat/completions', 200, 160, 950, 2900, 95, 190, 285, 'gemini-pro', '192.168.1.100', 'Mozilla/5.0', NULL, NULL, '2025-07-24 20:15:40')",
        
        // Claude API 请求 (user_service_api_id = 3)
        "INSERT INTO request_statistics (user_service_api_id, request_id, method, path, status_code, response_time_ms, request_size, response_size, tokens_prompt, tokens_completion, tokens_total, model_used, client_ip, user_agent, error_type, error_message, created_at) VALUES (3, 'demo-claude-3001', 'POST', '/v1/messages', 200, 200, 1300, 3800, 110, 280, 390, 'claude-3-sonnet', '10.0.0.50', 'python-requests/2.31.0', NULL, NULL, '2025-07-24 23:20:18')",
        
        "INSERT INTO request_statistics (user_service_api_id, request_id, method, path, status_code, response_time_ms, request_size, response_size, tokens_prompt, tokens_completion, tokens_total, model_used, client_ip, user_agent, error_type, error_message, created_at) VALUES (3, 'demo-claude-3002', 'POST', '/v1/messages', 200, 250, 1500, 4200, 140, 320, 460, 'claude-3-opus', '192.168.100.10', 'curl/8.1.0', NULL, NULL, '2025-07-24 22:10:25')",
        
        "INSERT INTO request_statistics (user_service_api_id, request_id, method, path, status_code, response_time_ms, request_size, response_size, tokens_prompt, tokens_completion, tokens_total, model_used, client_ip, user_agent, error_type, error_message, created_at) VALUES (3, 'demo-claude-3003', 'POST', '/v1/chat/completions', 200, 180, 1000, 3100, 85, 210, 295, 'claude-3-haiku', '172.16.0.200', 'PostmanRuntime/7.32.3', NULL, NULL, '2025-07-24 21:35:50')",
        
        "INSERT INTO request_statistics (user_service_api_id, request_id, method, path, status_code, response_time_ms, request_size, response_size, tokens_prompt, tokens_completion, tokens_total, model_used, client_ip, user_agent, error_type, error_message, created_at) VALUES (3, 'demo-claude-3004', 'POST', '/v1/messages', 401, 40, 700, 80, NULL, NULL, NULL, NULL, '192.168.1.100', 'Mozilla/5.0', 'auth_error', 'Invalid API key', '2025-07-24 20:45:30')",
        
        "INSERT INTO request_statistics (user_service_api_id, request_id, method, path, status_code, response_time_ms, request_size, response_size, tokens_prompt, tokens_completion, tokens_total, model_used, client_ip, user_agent, error_type, error_message, created_at) VALUES (3, 'demo-claude-3005', 'POST', '/v1/chat/completions', 200, 170, 1200, 3400, 105, 230, 335, 'claude-3-sonnet', '10.0.0.50', 'python-requests/2.31.0', NULL, NULL, '2025-07-24 19:30:15')",
        
        // 更多历史数据
        "INSERT INTO request_statistics (user_service_api_id, request_id, method, path, status_code, response_time_ms, request_size, response_size, tokens_prompt, tokens_completion, tokens_total, model_used, client_ip, user_agent, error_type, error_message, created_at) VALUES (1, 'demo-openai-1006', 'POST', '/v1/chat/completions', 200, 140, 1100, 3300, 90, 190, 280, 'gpt-4', '192.168.1.100', 'curl/8.1.0', NULL, NULL, '2025-07-24 18:45:20')",
        
        "INSERT INTO request_statistics (user_service_api_id, request_id, method, path, status_code, response_time_ms, request_size, response_size, tokens_prompt, tokens_completion, tokens_total, model_used, client_ip, user_agent, error_type, error_message, created_at) VALUES (2, 'demo-gemini-2005', 'POST', '/v1/completions', 200, 190, 1250, 3600, 130, 270, 400, 'gemini-1.5-pro', '10.0.0.50', 'python-requests/2.31.0', NULL, NULL, '2025-07-24 17:20:35')",
        
        "INSERT INTO request_statistics (user_service_api_id, request_id, method, path, status_code, response_time_ms, request_size, response_size, tokens_prompt, tokens_completion, tokens_total, model_used, client_ip, user_agent, error_type, error_message, created_at) VALUES (3, 'demo-claude-3006', 'POST', '/v1/messages', 200, 210, 1400, 3900, 125, 290, 415, 'claude-3-opus', '172.16.0.200', 'PostmanRuntime/7.32.3', NULL, NULL, '2025-07-24 16:15:42')",
        
        "INSERT INTO request_statistics (user_service_api_id, request_id, method, path, status_code, response_time_ms, request_size, response_size, tokens_prompt, tokens_completion, tokens_total, model_used, client_ip, user_agent, error_type, error_message, created_at) VALUES (1, 'demo-openai-1007', 'GET', '/v1/models', 200, 45, 120, 450, NULL, NULL, NULL, NULL, '192.168.100.10', 'Mozilla/5.0', NULL, NULL, '2025-07-24 15:50:28')",
        
        "INSERT INTO request_statistics (user_service_api_id, request_id, method, path, status_code, response_time_ms, request_size, response_size, tokens_prompt, tokens_completion, tokens_total, model_used, client_ip, user_agent, error_type, error_message, created_at) VALUES (2, 'demo-gemini-2006', 'POST', '/v1/chat/completions', 400, 35, 650, 120, NULL, NULL, NULL, 'gemini-pro', '192.168.1.100', 'curl/8.1.0', 'validation_error', 'Invalid request parameters', '2025-07-24 14:25:15')",
        
        "INSERT INTO request_statistics (user_service_api_id, request_id, method, path, status_code, response_time_ms, request_size, response_size, tokens_prompt, tokens_completion, tokens_total, model_used, client_ip, user_agent, error_type, error_message, created_at) VALUES (1, 'demo-openai-1008', 'POST', '/v1/embeddings', 200, 75, 550, 1100, NULL, NULL, NULL, 'text-embedding-ada-002', '10.0.0.50', 'python-requests/2.31.0', NULL, NULL, '2025-07-24 13:40:50')",
    ];
    
    for sql in sql_statements {
        manager.get_connection().execute_unprepared(sql).await?;
    }
    
    Ok(())
}
// 系统相关类型定义

// 健康检查响应
export interface HealthCheckResponse {
  status: 'healthy' | 'unhealthy'
  timestamp: string
  details: {
    healthy_servers: number
    total_servers: number
    avg_response_time_ms: number
  }
}

// 详细健康检查响应
export interface DetailedHealthResponse {
  status: 'healthy' | 'unhealthy'
  timestamp: string
  system: {
    total_servers: number
    healthy_servers: number
    unhealthy_servers: number
    active_tasks: number
    avg_response_time: string
    is_running: boolean
  }
  adapters: Record<string, any>
  load_balancers: string
}

// 系统信息
export interface SystemInfo {
  service: {
    name: string
    version: string
    build_time: string
    git_commit: string
  }
  runtime: {
    uptime_seconds: number
    rust_version: string
    target: string
    os_info?: string
    arch?: string
  }
  configuration: {
    server_port: number
    https_port: number
    workers: number
    database_url: string
  }
}

// 系统指标
export interface SystemMetrics {
  memory: {
    total: number
    used: number
    available: number
    usage_percent: number
  }
  cpu: {
    load_average: number[]
    cores: number
    usage_percent: number
  }
  network: {
    bytes_sent: number
    bytes_received: number
    connections_active: number
  }
  process: {
    pid: number
    threads: number
    file_descriptors: number
    uptime_seconds: number
  }
  timestamp: string
}

// 负载均衡器状态
export interface LoadBalancerStatus {
  status: 'active' | 'inactive'
  algorithms: string[]
  current_algorithm: string
  load_balancers: Record<string, {
    total_servers: number
    healthy_servers: number
    current_requests: number
  }>
}

// 服务器信息
export interface ServerInfo {
  id: string
  api_id: number
  upstream_type: string
  display_name: string
  host: string
  port: number
  use_tls: boolean
  weight: number
  is_healthy: boolean
  is_active: boolean
  response_time_ms: number
  requests_total: number
  requests_successful: number
  requests_failed: number
  rate_limit: number
  timeout_seconds: number
  created_at: string
  last_used?: string
}

// 服务器列表响应
export interface ServerListResponse {
  servers: ServerInfo[]
  total: number
  filters: {
    upstream_type?: string
    healthy?: boolean
  }
}

// 添加服务器请求
export interface AddServerRequest {
  upstream_type: string
  host: string
  port: number
  use_tls?: boolean
  weight?: number
  max_connections?: number
  timeout_ms?: number
}

// 服务器操作请求
export interface ServerActionRequest {
  server_id: string
  action: 'enable' | 'disable' | 'remove'
}

// 更改调度策略请求
export interface ChangeStrategyRequest {
  upstream_type: string
  strategy: 'round_robin' | 'weighted' | 'health_based'
}

// 负载均衡器指标
export interface LoadBalancerMetrics {
  metrics: Record<string, {
    total_servers: number
    healthy_servers: number
    unhealthy_servers: number
    success_rate: number
    servers: Array<{
      address: string
      weight: number
      is_healthy: boolean
      success_requests: number
      failed_requests: number
      avg_response_time_ms: number
      last_health_check: string
      use_tls: boolean
    }>
  }>
  timestamp: string
}
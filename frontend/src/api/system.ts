// 系统相关API

import { HttpClient } from '@/utils/http'
import { MockDataService, useMockData } from '@/utils/mockData'
import type {
  HealthCheckResponse,
  DetailedHealthResponse,
  SystemInfo,
  SystemMetrics,
  LoadBalancerStatus,
  ServerInfo,
  ServerListResponse,
  AddServerRequest,
  ServerActionRequest,
  ChangeStrategyRequest,
  LoadBalancerMetrics
} from '@/types'

export class SystemAPI {
  // ===== 健康检查 =====

  // 基础健康检查
  static async getHealth(): Promise<HealthCheckResponse> {
    if (useMockData) {
      return MockDataService.getHealthCheck()
    }
    return HttpClient.get('/health')
  }

  // 详细健康检查
  static async getDetailedHealth(): Promise<DetailedHealthResponse> {
    return HttpClient.get('/health/detailed')
  }

  // ===== 系统信息 =====

  // 获取系统信息
  static async getSystemInfo(): Promise<SystemInfo> {
    if (useMockData) {
      return MockDataService.getSystemInfo()
    }
    return HttpClient.get('/system/info')
  }

  // 获取系统指标
  static async getSystemMetrics(): Promise<SystemMetrics> {
    return HttpClient.get('/system/metrics')
  }

  // ===== 负载均衡器管理 =====

  // 获取负载均衡器状态
  static async getLoadBalancerStatus(): Promise<LoadBalancerStatus> {
    return HttpClient.get('/loadbalancer/status')
  }

  // 获取服务器列表
  static async getServers(params: {
    upstream_type?: string
    healthy?: boolean
  } = {}): Promise<ServerListResponse> {
    return HttpClient.get('/loadbalancer/servers', params)
  }

  // 添加新服务器
  static async addServer(data: AddServerRequest): Promise<{
    id: string
    success: boolean
    message: string
  }> {
    return HttpClient.post('/loadbalancer/servers', data)
  }

  // 服务器操作（启用/禁用/移除）
  static async serverAction(data: ServerActionRequest): Promise<{
    success: boolean
    message: string
    server_id: string
    action: string
  }> {
    return HttpClient.post('/loadbalancer/servers/action', data)
  }

  // 更改调度策略
  static async changeStrategy(data: ChangeStrategyRequest): Promise<{
    success: boolean
    message: string
    old_strategy: string
    new_strategy: string
  }> {
    return HttpClient.patch('/loadbalancer/strategy', data)
  }

  // 获取负载均衡器详细指标
  static async getLoadBalancerMetrics(): Promise<LoadBalancerMetrics> {
    return HttpClient.get('/loadbalancer/metrics')
  }

  // ===== 适配器管理 =====

  // 列出所有适配器
  static async getAdapters(): Promise<{
    adapters: Array<{
      id: number
      name: string
      display_name: string
      upstream_type: string
      base_url: string
      default_model: string
      max_tokens: number
      rate_limit: number
      timeout_seconds: number
      health_check_path: string
      auth_header_format: string
      status: string
      supported_endpoints: number
      endpoints: string[]
      version: string
      created_at: string
      updated_at: string
    }>
    total: number
    timestamp: string
  }> {
    return HttpClient.get('/adapters')
  }

  // 获取适配器统计信息
  static async getAdapterStats(): Promise<{
    summary: {
      total_adapters: number
      total_endpoints: number
      adapter_types: number
      total_active_configs: number
    }
    by_type: Record<string, {
      adapters: number
      endpoints: number
      active_configs: number
      names: string[]
    }>
    detailed_stats: Record<string, {
      id: number
      display_name: string
      api_format: string
      base_url: string
      supported_endpoints: number
      active_configurations: number
      runtime_info: {
        upstream_type: string
        endpoints: string[]
      }
      health_status: {
        status: string
        last_check: string
        response_time_ms: number
        success_rate: number
        healthy_servers: number
        total_servers: number
        is_healthy: boolean
      }
      rate_limit: number
      timeout_seconds: number
      last_updated: string
    }>
    timestamp: string
  }> {
    return HttpClient.get('/adapters/stats')
  }

  // ===== 配置管理 =====

  // 获取系统配置
  static async getConfig(): Promise<Record<string, any>> {
    return HttpClient.get('/system/config')
  }

  // 更新系统配置
  static async updateConfig(config: Record<string, any>): Promise<{
    success: boolean
    message: string
    updated_keys: string[]
  }> {
    return HttpClient.put('/system/config', config)
  }

  // 重载配置
  static async reloadConfig(): Promise<{
    success: boolean
    message: string
    reloaded_at: string
  }> {
    return HttpClient.post('/system/config/reload')
  }

  // ===== 日志管理 =====

  // 获取系统日志
  static async getSystemLogs(params: {
    level?: 'debug' | 'info' | 'warn' | 'error'
    limit?: number
    offset?: number
    start_time?: string
    end_time?: string
  } = {}): Promise<{
    logs: Array<{
      timestamp: string
      level: string
      message: string
      module: string
      details?: any
    }>
    total: number
    has_more: boolean
  }> {
    return HttpClient.get('/system/logs', params)
  }

  // 下载系统日志
  static async downloadSystemLogs(params: {
    start_time?: string
    end_time?: string
    level?: string
  } = {}): Promise<void> {
    const filename = `system_logs_${new Date().toISOString().split('T')[0]}.txt`
    return HttpClient.download('/system/logs/download', params, filename)
  }

  // ===== 系统操作 =====

  // 系统重启
  static async restartSystem(): Promise<{
    success: boolean
    message: string
    restart_id: string
  }> {
    return HttpClient.post('/system/restart')
  }

  // 清理缓存
  static async clearCache(cacheType?: 'all' | 'auth' | 'stats' | 'health'): Promise<{
    success: boolean
    message: string
    cleared_items: number
  }> {
    return HttpClient.post('/system/cache/clear', { cache_type: cacheType || 'all' })
  }

  // 垃圾回收
  static async runGarbageCollection(): Promise<{
    success: boolean
    message: string
    memory_freed: number
  }> {
    return HttpClient.post('/system/gc')
  }

  // ===== 备份与恢复 =====

  // 创建系统备份
  static async createBackup(): Promise<{
    success: boolean
    backup_id: string
    filename: string
    size: number
    created_at: string
  }> {
    return HttpClient.post('/system/backup')
  }

  // 获取备份列表
  static async getBackups(): Promise<Array<{
    backup_id: string
    filename: string
    size: number
    created_at: string
    status: 'completed' | 'failed' | 'in_progress'
  }>> {
    return HttpClient.get('/system/backups')
  }

  // 下载备份文件
  static async downloadBackup(backupId: string): Promise<void> {
    return HttpClient.download(`/system/backups/${backupId}/download`)
  }

  // 恢复系统
  static async restoreSystem(backupId: string): Promise<{
    success: boolean
    message: string
    restore_id: string
  }> {
    return HttpClient.post(`/system/restore/${backupId}`)
  }
}
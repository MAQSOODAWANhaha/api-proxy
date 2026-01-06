import { ProviderKey } from '../../../lib/api'

/** 账号 API Key 数据结构 - 与后端保持一致 */
export interface LocalProviderKey extends Omit<ProviderKey, 'status'> {
  // 为了兼容现有UI，添加一些别名
  keyName: string // 映射到 name
  keyValue: string // 映射到 api_key
  status: 'active' | 'disabled' // 基于 is_active 转换，不再用于筛选
  createdAt: string // 映射到 created_at
  requestLimitPerMinute: number // 映射到 max_requests_per_minute
  tokenLimitPromptPerMinute: number // 映射到 max_tokens_prompt_per_minute
  requestLimitPerDay: number // 映射到 max_requests_per_day
  healthStatus: string // 健康状态（用于显示和内部逻辑）
  cost: number // 从 usage.total_cost 映射
  usage: {
    total_requests: number
    successful_requests: number
    failed_requests: number
    success_rate: number
    total_tokens: number
    total_cost: number
    avg_response_time: number
    last_used_at?: string
  } // 使用完整的usage对象结构
  rateLimitRemainingSeconds?: number // 限流剩余时间（秒）
  project_id?: string // Gemini OAuth extra project scope
  health_status_detail?: string // 健康状态详情（JSON字符串）
}

export interface ProviderKeyFormState {
  provider: string
  provider_type_id: number
  keyName: string
  keyValue: string
  auth_type: string
  weight: number
  requestLimitPerMinute: number
  tokenLimitPromptPerMinute: number
  requestLimitPerDay: number
  status: 'active' | 'disabled'
  project_id?: string
}

export type ProviderKeyEditFormState = ProviderKeyFormState & { id: number }

/** 弹窗类型 */
export type DialogType = 'add' | 'edit' | 'delete' | 'stats' | null

/** 统计对话框趋势点 */
export interface ProviderKeyTrendPoint {
  date: string
  requests: number
  cost: number
}

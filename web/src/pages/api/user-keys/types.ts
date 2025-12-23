import { UserServiceApiKey } from '../../../lib/api'

// 使用API中定义的类型，并添加额外需要的字段
export interface ApiKey extends UserServiceApiKey {
  scheduling_strategy?: string
  user_provider_keys_ids?: number[]
  retry_count?: number
  timeout_seconds?: number
  max_request_per_min?: number
  max_requests_per_day?: number
  max_tokens_per_day?: number
  max_cost_per_day?: number
}

/** 用户提供商密钥 */
export interface UserProviderKey {
  id: number
  name: string
  display_name: string
}

/** 弹窗类型 */
export type DialogType = 'add' | 'edit' | 'delete' | 'stats' | null

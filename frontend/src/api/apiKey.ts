import request from './index'

// Define the ProviderKey type to match backend API response
export interface ProviderKey {
  id: number
  user_id: number
  provider_type: string
  provider_display_name: string
  name: string
  api_key_prefix: string
  weight: number | null
  max_requests_per_minute: number | null
  max_tokens_per_day: number | null
  used_tokens_today: number | null
  status: 'active' | 'inactive'
  created_at: string
  updated_at: string
  last_used: string | null
  // Frontend display fields
  provider?: 'openai' | 'gemini' | 'claude'
  apiKey?: string
  lastUsed?: string
}

// Define the create request type
export interface CreateProviderKeyRequest {
  user_id: number
  provider_type_id: number
  name: string
  api_key: string
  weight?: number
  max_requests_per_minute?: number
  max_tokens_per_day?: number
}

// Define the update request type
export interface UpdateProviderKeyRequest {
  name?: string
  api_key?: string
  weight?: number
  max_requests_per_minute?: number
  max_tokens_per_day?: number
  is_active?: boolean
}

// API response wrapper
export interface ApiResponse<T> {
  success?: boolean
  data?: T
  provider_keys?: T
  message?: string
  pagination?: {
    page: number
    limit: number
    total: number
    pages: number
  }
}

// --- API Functions ---

// Get list of provider keys
export async function getProviderKeys(): Promise<{ data: ProviderKey[] }> {
  try {
    const response = await request.get<ApiResponse<ProviderKey[]>>('/provider-keys')
    let data = response.data.provider_keys || response.data.data || []
    
    // Transform backend data to frontend format for backward compatibility
    data = data.map((key: ProviderKey) => ({
      ...key,
      provider: key.provider_type as 'openai' | 'gemini' | 'claude',
      apiKey: key.api_key_prefix,
      lastUsed: key.last_used ? new Date(key.last_used).toISOString().split('T')[0] : '',
      weight: key.weight || 1
    }))
    
    return { data }
  } catch (error) {
    console.error('Failed to get provider keys:', error)
    throw error
  }
}

// Add a new provider key
export async function addProviderKey(data: {
  name: string
  provider: 'openai' | 'gemini' | 'claude'
  apiKey: string
  weight: number
  status: 'active' | 'inactive'
}): Promise<{ data: any }> {
  try {
    // Map frontend types to backend types
    const providerTypeMap = {
      'openai': 1,
      'gemini': 2,
      'claude': 3
    }
    
    const requestData: CreateProviderKeyRequest = {
      user_id: 1, // TODO: Get from user context
      provider_type_id: providerTypeMap[data.provider],
      name: data.name,
      api_key: data.apiKey,
      weight: data.weight || 1,
      max_requests_per_minute: 100,
      max_tokens_per_day: 10000
    }
    
    const response = await request.post<ApiResponse<any>>('/provider-keys', requestData)
    return { data: response.data }
  } catch (error) {
    console.error('Failed to create provider key:', error)
    throw error
  }
}

// Update an existing provider key
export async function updateProviderKey(data: ProviderKey): Promise<{ data: ProviderKey }> {
  try {
    const requestData: UpdateProviderKeyRequest = {
      name: data.name,
      weight: data.weight || undefined,
      max_requests_per_minute: data.max_requests_per_minute || undefined,
      max_tokens_per_day: data.max_tokens_per_day || undefined,
      is_active: data.status === 'active'
    }
    
    const response = await request.put<ApiResponse<ProviderKey>>(`/provider-keys/${data.id}`, requestData)
    return { data: response.data.data || data }
  } catch (error) {
    console.error('Failed to update provider key:', error)
    throw error
  }
}

// Delete a provider key
export async function deleteProviderKey(id: number): Promise<void> {
  try {
    await request.delete(`/provider-keys/${id}`)
  } catch (error) {
    console.error('Failed to delete provider key:', error)
    throw error
  }
}

// Get single provider key
export async function getProviderKey(id: number): Promise<{ data: ProviderKey }> {
  try {
    const response = await request.get<ProviderKey>(`/provider-keys/${id}`)
    return { data: response.data }
  } catch (error) {
    console.error('Failed to get provider key:', error)
    throw error
  }
}


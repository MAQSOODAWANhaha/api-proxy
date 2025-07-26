import request from './index'

// Define the ServiceKey type to match backend API response
export interface ServiceKey {
  id: number
  name: string
  key_prefix: string
  user_id: number
  description?: string
  status: 'active' | 'inactive'
  scopes: string[]
  usage_count: number
  created_at: string
  expires_at?: string
  last_used_at?: string
  key?: string // Only available when creating
}

// Create API Key Request type
export interface CreateServiceKeyRequest {
  user_id: number
  provider_type_id: number // 1=OpenAI, 2=Gemini, 3=Claude
  name: string
  description?: string
  expires_in_days?: number
  scopes?: string[]
}

// Update API Key Request type
export interface UpdateServiceKeyRequest {
  name?: string
  description?: string
  is_active?: boolean
}

// API response wrapper
export interface ApiResponse<T> {
  success?: boolean
  data?: T
  api_keys?: T
  api_key?: ServiceKey
  message?: string
  pagination?: {
    page: number
    limit: number
    total: number
    pages: number
  }
}

// Provider Type interface
export interface ProviderType {
  id: number
  name: string
  display_name: string
  base_url: string
  api_format: string
  default_model?: string
  is_active: boolean
}

// --- API Functions ---

// Get list of available provider types
export async function getProviderTypes(): Promise<{ data: ProviderType[] }> {
  try {
    const response = await request.get<ApiResponse<ProviderType[]>>('/provider-types')
    const data = response.data.data || []
    return { data }
  } catch (error) {
    console.error('Failed to get provider types:', error)
    throw error
  }
}

// Get list of service keys
export async function getServiceKeys(): Promise<{ data: ServiceKey[] }> {
  try {
    const response = await request.get<ApiResponse<ServiceKey[]>>('/api-keys')
    const data = response.data.api_keys || response.data.data || []
    return { data }
  } catch (error) {
    console.error('Failed to get service keys:', error)
    throw error
  }
}

// Add a new service key
export async function addServiceKey(data: {
  name: string
  provider_type_id: number
  description?: string
  expires_in_days?: number
}): Promise<{ data: ServiceKey }> {
  try {
    const requestData: CreateServiceKeyRequest = {
      user_id: 1, // TODO: Get from user context
      provider_type_id: data.provider_type_id,
      name: data.name,
      description: data.description,
      expires_in_days: data.expires_in_days,
      scopes: ['api:access', 'ai:chat', 'ai:completion']
    }
    
    const response = await request.post<ApiResponse<ServiceKey>>('/api-keys', requestData)
    const apiKey = response.data.api_key || response.data.data
    if (!apiKey) {
      throw new Error('No API key returned from server')
    }
    return { data: apiKey }
  } catch (error) {
    console.error('Failed to create service key:', error)
    throw error
  }
}

// Update an existing service key
export async function updateServiceKey(data: ServiceKey): Promise<{ data: ServiceKey }> {
  try {
    const requestData: UpdateServiceKeyRequest = {
      name: data.name,
      description: data.description,
      is_active: data.status === 'active'
    }
    
    const response = await request.put<ApiResponse<ServiceKey>>(`/api-keys/${data.id}`, requestData)
    return { data: response.data.data || data }
  } catch (error) {
    console.error('Failed to update service key:', error)
    throw error
  }
}

// Revoke a service key
export async function revokeServiceKey(id: number): Promise<void> {
  try {
    await request.post(`/api-keys/${id}/revoke`)
  } catch (error) {
    console.error('Failed to revoke service key:', error)
    throw error
  }
}

// Delete a service key (same as revoke for now)
export async function deleteServiceKey(id: number): Promise<void> {
  return revokeServiceKey(id)
}

// Get single service key
export async function getServiceKey(id: number): Promise<{ data: ServiceKey }> {
  try {
    const response = await request.get<ServiceKey>(`/api-keys/${id}`)
    return { data: response.data }
  } catch (error) {
    console.error('Failed to get service key:', error)
    throw error
  }
}

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
  name: string
  description?: string
  expires_in_days?: number
  scopes?: string[]
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

// --- API Functions ---

// Get list of service keys
export async function getServiceKeys(): Promise<{ data: ServiceKey[] }> {
  try {
    const response = await request.get<ApiResponse<ServiceKey[]>>('/api/api-keys')
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
  description?: string
  expires_in_days?: number
}): Promise<{ data: ServiceKey }> {
  try {
    const requestData: CreateServiceKeyRequest = {
      user_id: 1, // TODO: Get from user context
      name: data.name,
      description: data.description,
      expires_in_days: data.expires_in_days,
      scopes: ['api:access', 'ai:chat', 'ai:completion']
    }
    
    const response = await request.post<ApiResponse<ServiceKey>>('/api/api-keys', requestData)
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

// Update an existing service key (revoke functionality)
export async function updateServiceKey(data: ServiceKey): Promise<{ data: ServiceKey }> {
  // Since backend only supports revoke, we'll implement this as a revoke operation
  if (data.status === 'inactive') {
    await revokeServiceKey(data.id)
    return { data }
  }
  // For other updates, we would need additional backend endpoints
  return { data }
}

// Revoke a service key
export async function revokeServiceKey(id: number): Promise<void> {
  try {
    await request.post(`/api/api-keys/${id}/revoke`)
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
    const response = await request.get<ServiceKey>(`/api/api-keys/${id}`)
    return { data: response.data }
  } catch (error) {
    console.error('Failed to get service key:', error)
    throw error
  }
}

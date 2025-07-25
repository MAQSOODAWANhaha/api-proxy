import request from './index'
import type { AxiosPromise } from 'axios'

// Define the ProviderKey type
export interface ProviderKey {
  id: number
  name: string
  provider: string // This will be mapped from provider_type_id
  api_key: string
  weight: number
  status: string
  lastUsed: string // This might not be available from the new endpoint
}

// --- API Functions ---

// Get list of provider keys
export function getProviderKeys(): AxiosPromise<ProviderKey[]> {
  return request({
    url: '/provider-keys',
    method: 'get',
  })
}

// Add a new provider key
export function addProviderKey(data: Omit<ProviderKey, 'id' | 'lastUsed'>): AxiosPromise<ProviderKey> {
  return request({
    url: '/provider-keys',
    method: 'post',
    data,
  })
}

// Update an existing provider key
export function updateProviderKey(data: ProviderKey): AxiosPromise<ProviderKey> {
  return request({
    url: `/provider-keys/${data.id}`,
    method: 'put',
    data,
  })
}

// Delete a provider key
export function deleteProviderKey(id: number): AxiosPromise<void> {
  return request({
    url: `/provider-keys/${id}`,
    method: 'delete',
  })
}


import request from './index'
import type { AxiosPromise } from 'axios'

// Define the ProviderKey type
export interface ProviderKey {
  id: number
  name: string
  provider: 'openai' | 'gemini' | 'claude'
  apiKey: string
  weight: number
  status: 'active' | 'inactive'
  lastUsed: string
}

// Mock database
let mockKeys: ProviderKey[] = [
  { id: 1, name: 'My OpenAI Key 1', provider: 'openai', apiKey: 'sk-abc...', weight: 10, status: 'active', lastUsed: '2025-07-21' },
  { id: 2, name: 'My Gemini Key', provider: 'gemini', apiKey: 'gem-xyz...', weight: 5, status: 'inactive', lastUsed: '2025-07-20' },
  { id: 3, name: 'Claude Sonnet Key', provider: 'claude', apiKey: 'cla-123...', weight: 10, status: 'active', lastUsed: '2025-07-22' },
]
let nextId = 4

// --- API Functions ---

// Get list of provider keys
export function getProviderKeys(): AxiosPromise<ProviderKey[]> {
  return new Promise((resolve) => {
    setTimeout(() => {
      resolve({ data: mockKeys } as any)
    }, 300)
  })
}

// Add a new provider key
export function addProviderKey(data: Omit<ProviderKey, 'id' | 'lastUsed'>): AxiosPromise<ProviderKey> {
  return new Promise((resolve) => {
    setTimeout(() => {
      const newKey: ProviderKey = {
        ...data,
        id: nextId++,
        lastUsed: new Date().toISOString().split('T')[0],
      }
      mockKeys.push(newKey)
      resolve({ data: newKey } as any)
    }, 300)
  })
}

// Update an existing provider key
export function updateProviderKey(data: ProviderKey): AxiosPromise<ProviderKey> {
  return new Promise((resolve, reject) => {
    setTimeout(() => {
      const index = mockKeys.findIndex(k => k.id === data.id)
      if (index !== -1) {
        mockKeys[index] = { ...mockKeys[index], ...data }
        resolve({ data: mockKeys[index] } as any)
      } else {
        reject(new Error('Key not found'))
      }
    }, 300)
  })
}

// Delete a provider key
export function deleteProviderKey(id: number): AxiosPromise<void> {
  return new Promise((resolve, reject) => {
    setTimeout(() => {
      const index = mockKeys.findIndex(k => k.id === id)
      if (index !== -1) {
        mockKeys.splice(index, 1)
        resolve({} as any)
      } else {
        reject(new Error('Key not found'))
      }
    }, 300)
  })
}

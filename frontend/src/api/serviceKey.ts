import type { AxiosPromise } from 'axios'

// Define the ServiceKey type
export interface ServiceKey {
  id: number
  name: string
  provider: 'openai' | 'gemini' | 'claude'
  apiKey: string // The key we generate for the user
  strategy: 'round_robin' | 'weighted' | 'health_best'
  status: 'active' | 'inactive'
  totalRequests: number
  successfulRequests: number
}

// Mock database
let mockServiceKeys: ServiceKey[] = [
  { id: 1, name: 'My OpenAI Service', provider: 'openai', apiKey: 'proxy-abc-123', strategy: 'round_robin', status: 'active', totalRequests: 10520, successfulRequests: 10500 },
  { id: 2, name: 'My Gemini Service', provider: 'gemini', apiKey: 'proxy-def-456', strategy: 'health_best', status: 'active', totalRequests: 5430, successfulRequests: 5421 },
]
let nextId = 3

// --- API Functions ---

export function getServiceKeys(): AxiosPromise<ServiceKey[]> {
  return new Promise((resolve) => {
    setTimeout(() => {
      resolve({ data: mockServiceKeys } as any)
    }, 300)
  })
}

export function addServiceKey(data: Omit<ServiceKey, 'id' | 'totalRequests' | 'successfulRequests'>): AxiosPromise<ServiceKey> {
  return new Promise((resolve) => {
    setTimeout(() => {
      const newKey: ServiceKey = {
        ...data,
        id: nextId++,
        apiKey: `proxy-${Math.random().toString(36).substring(2, 9)}`,
        totalRequests: 0,
        successfulRequests: 0,
      }
      mockServiceKeys.push(newKey)
      resolve({ data: newKey } as any)
    }, 300)
  })
}

export function updateServiceKey(data: ServiceKey): AxiosPromise<ServiceKey> {
  return new Promise((resolve, reject) => {
    setTimeout(() => {
      const index = mockServiceKeys.findIndex(k => k.id === data.id)
      if (index !== -1) {
        mockServiceKeys[index] = { ...mockServiceKeys[index], ...data }
        resolve({ data: mockServiceKeys[index] } as any)
      } else {
        reject(new Error('Key not found'))
      }
    }, 300)
  })
}

export function deleteServiceKey(id: number): AxiosPromise<void> {
  return new Promise((resolve, reject) => {
    setTimeout(() => {
      const index = mockServiceKeys.findIndex(k => k.id === id)
      if (index !== -1) {
        mockServiceKeys.splice(index, 1)
        resolve({} as any)
      } else {
        reject(new Error('Key not found'))
      }
    }, 300)
  })
}

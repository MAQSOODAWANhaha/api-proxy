import type { AxiosPromise } from 'axios'

// Define the User Profile type
export interface UserProfile {
  username: string
  email: string
  lastLogin: string
}

// Mock API response
const mockUserProfile: UserProfile = {
  username: 'admin',
  email: 'admin@example.com',
  lastLogin: '2025-07-22 10:30:00',
}

export function getUserProfile(): AxiosPromise<UserProfile> {
  return new Promise((resolve) => {
    setTimeout(() => {
      resolve({ data: mockUserProfile } as any)
    }, 300)
  })
}

export function updatePassword(data: any): AxiosPromise<void> {
  return new Promise((resolve) => {
    setTimeout(() => {
      console.log('Password update request:', data)
      resolve({} as any)
    }, 500)
  })
}

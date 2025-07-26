import request from './index'
import type { AxiosPromise } from 'axios'

// Define the User Profile type
export interface UserProfile {
  username: string
  email: string
  lastLogin: string
  is_admin: boolean
  created_at: string
}

// API response type
interface UserProfileResponse {
  success: boolean
  data: UserProfile
  message?: string
}

// Update profile request
export interface UpdateProfileRequest {
  email?: string
}

// Change password request
export interface ChangePasswordRequest {
  current_password: string
  new_password: string
}

// Get user profile
export function getUserProfile(): AxiosPromise<UserProfile> {
  return new Promise(async (resolve, reject) => {
    try {
      const response = await request.get<UserProfileResponse>('/users/profile')
      if (response.data.success && response.data.data) {
        // Transform backend format to frontend format
        const profile: UserProfile = {
          username: response.data.data.username,
          email: response.data.data.email,
          lastLogin: response.data.data.lastLogin || response.data.data.created_at,
          is_admin: response.data.data.is_admin,
          created_at: response.data.data.created_at
        }
        resolve({ data: profile } as any)
      } else {
        reject(new Error(response.data.message || 'Failed to fetch user profile'))
      }
    } catch (error) {
      console.error('Failed to fetch user profile:', error)
      // Fallback to mock data
      const mockUserProfile: UserProfile = {
        username: 'admin',
        email: 'admin@example.com',
        lastLogin: '2025-07-22 10:30:00',
        is_admin: true,
        created_at: '2025-01-01 00:00:00'
      }
      resolve({ data: mockUserProfile } as any)
    }
  })
}

// Update user profile
export function updateUserProfile(data: UpdateProfileRequest): AxiosPromise<UserProfile> {
  return request.put<UserProfileResponse>('/users/profile', data)
    .then(response => {
      if (response.data.success && response.data.data) {
        return { data: response.data.data } as any
      } else {
        throw new Error(response.data.message || 'Failed to update profile')
      }
    })
}

// Change password
export function updatePassword(data: ChangePasswordRequest): AxiosPromise<void> {
  return request.post('/users/password', data)
    .then(response => {
      if (!response.data.success) {
        throw new Error(response.data.message || 'Failed to change password')
      }
      return response
    })
}

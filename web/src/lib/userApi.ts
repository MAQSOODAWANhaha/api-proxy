import { apiClient, type ApiResponse } from './api';

/**
 * 用户管理 API 接口
 */

// 用户数据类型
export interface User {
  id: number;
  username: string;
  email: string;
  is_active: boolean;
  is_admin: boolean;
  created_at: string;
  updated_at: string;
  last_login?: string;
}

// 用户查询参数
export interface UserQueryParams {
  page?: number;
  limit?: number;
  search?: string;
  is_active?: boolean;
  is_admin?: boolean;
  sort?: 'created_at' | 'updated_at' | 'username' | 'email';
  order?: 'asc' | 'desc';
}

// 创建用户请求
export interface CreateUserRequest {
  username: string;
  email: string;
  password: string;
  is_admin?: boolean;
}

// 更新用户请求
export interface UpdateUserRequest {
  username?: string;
  email?: string;
  password?: string;
  is_active?: boolean;
  is_admin?: boolean;
}

// 用户API接口
export const userApi = {
  // 获取用户列表
  async getUsers(params: UserQueryParams = {}): Promise<ApiResponse<{ users: User[] }>> {
    return apiClient.get('/users', params as Record<string, any>);
  },

  // 获取单个用户
  async getUser(id: number): Promise<ApiResponse<User>> {
    return apiClient.get(`/users/${id}`);
  },

  // 创建用户
  async createUser(userData: CreateUserRequest): Promise<ApiResponse<User>> {
    return apiClient.post('/users', userData);
  },

  // 更新用户
  async updateUser(id: number, userData: UpdateUserRequest): Promise<ApiResponse<User>> {
    return apiClient.put(`/users/${id}`, userData);
  },

  // 删除用户
  async deleteUser(id: number): Promise<ApiResponse<null>> {
    return apiClient.delete(`/users/${id}`);
  },

  // 批量删除用户
  async batchDeleteUsers(ids: number[]): Promise<ApiResponse<null>> {
    return apiClient.request('/users', { method: 'DELETE', body: { ids } });
  },

  // 切换用户状态
  async toggleUserStatus(id: number): Promise<ApiResponse<User>> {
    return apiClient.request(`/users/${id}/toggle-status`, { method: 'PATCH' });
  },

  // 重置用户密码
  async resetUserPassword(id: number, newPassword: string): Promise<ApiResponse<null>> {
    return apiClient.request(`/users/${id}/reset-password`, { method: 'PATCH', body: { new_password: newPassword } });
  },
};

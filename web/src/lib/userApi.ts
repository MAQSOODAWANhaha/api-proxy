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
  // 统计数据
  total_requests: number;
  total_cost: number;
  total_tokens: number;
}

// 用户统计数据（用户管理页顶部统计卡片）
export interface UsersStats {
  total: number;
  active: number;
  admin: number;
  inactive: number;
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
  async getUsers(params: UserQueryParams = {}): Promise<ApiResponse<User[]>> {
    const raw = await apiClient.get<User[] | { users: User[] }>(
      '/users',
      params as Record<string, any>,
    );

    if (!raw.success) {
      return raw as ApiResponse<User[]>;
    }

    const users = Array.isArray(raw.data) ? raw.data : raw.data?.users ?? [];
    return { ...raw, data: users };
  },

  // 获取用户统计（避免通过 limit=1000 拉全量列表）
  async getUsersStats(): Promise<ApiResponse<UsersStats>> {
    return apiClient.get('/users/stats');
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

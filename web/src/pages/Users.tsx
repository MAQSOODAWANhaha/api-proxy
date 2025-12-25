/**
 * Users.tsx
 * 用户管理页：完整的用户数据管理、搜索过滤和分页功能
 */

import React, { useState, useEffect, useCallback } from 'react'
import {
  Search,
  Plus,
  Edit,
  Trash2,
  Eye,
  RefreshCw,
  User,
  Mail,
  Calendar,
  Shield,
  Activity,
  Clock,
  ChevronLeft,
  ChevronRight,
  Users,
  BarChart3,
  ToggleLeft,
  ToggleRight,
  Key,
  AlertCircle
} from 'lucide-react'
import { StatCard } from '../components/common/StatCard'
import FilterSelect from '../components/common/FilterSelect'
import ModernSelect from '../components/common/ModernSelect'
import {
  userApi,
  User as UserType,
  UserQueryParams,
  CreateUserRequest,
  UpdateUserRequest,
} from '../lib/userApi'
import { LoadingSpinner, LoadingState } from '@/components/ui/loading'
import { Skeleton } from '@/components/ui/skeleton'

/** 弹窗类型 */
type DialogType = 'add' | 'edit' | 'delete' | 'details' | 'resetPassword' | 'batchDelete' | null

/** 用户页面统计数据 */
interface UserStats {
  total: number
  active: number
  admin: number
  inactive: number
}

/** 页面主组件 */
const UsersPage: React.FC = () => {
  // 数据状态
  const [users, setUsers] = useState<UserType[]>([])
  const [loading, setLoading] = useState(true)
  const [statsLoading, setStatsLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [stats, setStats] = useState<UserStats>({ total: 0, active: 0, admin: 0, inactive: 0 })
  
  // 查询状态
  const [searchTerm, setSearchTerm] = useState('')
  const [isActiveFilter, setIsActiveFilter] = useState<'all' | 'true' | 'false'>('all')
  const [isAdminFilter, setIsAdminFilter] = useState<'all' | 'true' | 'false'>('all')
  const sortField: UserQueryParams['sort'] = 'created_at'
  const sortOrder: UserQueryParams['order'] = 'desc'
  
  // 分页状态
  const [currentPage, setCurrentPage] = useState(1)
  const [pageSize, setPageSize] = useState(10)
  const [totalPages, setTotalPages] = useState(0)
  const [totalUsers, setTotalUsers] = useState(0)
  
  // 弹窗状态
  const [selectedUser, setSelectedUser] = useState<UserType | null>(null)
  const [selectedUsers, setSelectedUsers] = useState<number[]>([])
  const [dialogType, setDialogType] = useState<DialogType>(null)

  // 加载统计数据（避免分页/筛选时重复请求）
  const loadStats = useCallback(async () => {
    setStatsLoading(true)
    try {
      const statsResponse = await userApi.getUsersStats()
      if (statsResponse.success && statsResponse.data) {
        setStats(statsResponse.data)
      } else if (!statsResponse.success) {
        setError(statsResponse.error?.message || statsResponse.message || '加载用户统计失败')
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : '加载用户统计失败')
    } finally {
      setStatsLoading(false)
    }
  }, [])

  // 加载用户数据
  const loadUsers = useCallback(async () => {
    try {
      setLoading(true)
      setError(null)
      
      const params: UserQueryParams = {
        page: currentPage,
        limit: pageSize,
        sort: sortField,
        order: sortOrder,
      }
      
      if (searchTerm.trim()) {
        params.search = searchTerm.trim()
      }
      
      if (isActiveFilter !== 'all') {
        params.is_active = isActiveFilter === 'true'
      }
      
      if (isAdminFilter !== 'all') {
        params.is_admin = isAdminFilter === 'true'
      }

      const response = await userApi.getUsers(params)
      
      if (response.success) {
        const usersList = response.data ?? []
        setUsers(usersList)
        setTotalPages(response.pagination?.pages || 0)
        setTotalUsers(response.pagination?.total || usersList.length)
      } else {
        setError(response.error?.message || response.message || '加载用户数据失败')
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : '加载用户数据失败')
      console.error('加载用户失败:', err)
    } finally {
      setLoading(false)
    }
  }, [currentPage, pageSize, searchTerm, isActiveFilter, isAdminFilter, sortField, sortOrder])

  // 初始加载和参数变化时重新加载
  useEffect(() => {
    loadUsers()
  }, [loadUsers])

  // 初始加载统计数据
  useEffect(() => {
    loadStats()
  }, [loadStats])

  // 重置页码当过滤条件改变时
  useEffect(() => {
    setCurrentPage(1)
  }, [searchTerm, isActiveFilter, isAdminFilter])

  // 格式化时间
  const formatLastLogin = (timestamp?: string) => {
    if (!timestamp) return '从未登录'
    
    const date = new Date(timestamp)
    const now = new Date()
    const diffInHours = Math.floor((now.getTime() - date.getTime()) / (1000 * 60 * 60))
    
    if (diffInHours < 1) return '刚刚'
    if (diffInHours < 24) return `${diffInHours}小时前`
    if (diffInHours < 168) return `${Math.floor(diffInHours / 24)}天前`
    return date.toLocaleDateString()
  }

  // 格式化日期
  const formatDate = (timestamp: string) => {
    return new Date(timestamp).toLocaleDateString()
  }

  const pageLoading = loading || statsLoading

  // 渲染用户状态
  const renderUserStatus = (isActive: boolean) => {
    if (isActive) {
      return (
        <span className="inline-flex items-center px-2 py-1 rounded-full text-xs font-medium bg-emerald-50 text-emerald-600 ring-1 ring-emerald-200">
          <Activity size={10} className="mr-1" />
          活跃
        </span>
      )
    }
    return (
      <span className="inline-flex items-center px-2 py-1 rounded-full text-xs font-medium bg-red-50 text-red-600 ring-1 ring-red-200">
        <Activity size={10} className="mr-1" />
        非活跃
      </span>
    )
  }

  // 渲染用户角色
  const renderUserRole = (isAdmin: boolean) => {
    if (isAdmin) {
      return (
        <span className="inline-flex items-center px-2 py-1 rounded-full text-xs font-medium bg-red-50 text-red-600 ring-1 ring-red-200">
          <Shield size={10} className="mr-1" />
          管理员
        </span>
      )
    }
    return (
      <span className="inline-flex items-center px-2 py-1 rounded-full text-xs font-medium bg-neutral-50 text-neutral-600 ring-1 ring-neutral-200">
        <User size={10} className="mr-1" />
        普通用户
      </span>
    )
  }

  // 切换用户状态
  const handleToggleStatus = async (user: UserType) => {
    try {
      const response = await userApi.toggleUserStatus(user.id)
      if (response.success) {
        await Promise.all([loadUsers(), loadStats()]) // 重新加载数据与统计
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : '切换用户状态失败')
    }
  }

  // 删除用户
  const handleDeleteUser = async () => {
    if (!selectedUser) return
    
    try {
      const response = await userApi.deleteUser(selectedUser.id)
      if (response.success) {
        setDialogType(null)
        setSelectedUser(null)
        await Promise.all([loadUsers(), loadStats()])
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : '删除用户失败')
    }
  }

  // 批量删除用户
  const handleBatchDelete = async () => {
    if (selectedUsers.length === 0) return
    
    try {
      const response = await userApi.batchDeleteUsers(selectedUsers)
      if (response.success) {
        setDialogType(null)
        setSelectedUsers([])
        await Promise.all([loadUsers(), loadStats()])
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : '批量删除用户失败')
    }
  }

  // 创建用户
  const handleCreateUser = async (userData: CreateUserRequest) => {
    try {
      const response = await userApi.createUser(userData)
      if (response.success) {
        setDialogType(null)
        await Promise.all([loadUsers(), loadStats()])
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : '创建用户失败')
    }
  }

  // 更新用户
  const handleUpdateUser = async (userData: UpdateUserRequest) => {
    if (!selectedUser) return
    
    try {
      const response = await userApi.updateUser(selectedUser.id, userData)
      if (response.success) {
        setDialogType(null)
        setSelectedUser(null)
        await Promise.all([loadUsers(), loadStats()])
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : '更新用户失败')
    }
  }

  // 重置密码
  const handleResetPassword = async (newPassword: string) => {
    if (!selectedUser) return
    
    try {
      const response = await userApi.resetUserPassword(selectedUser.id, newPassword)
      if (response.success) {
        setDialogType(null)
        setSelectedUser(null)
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : '重置密码失败')
    }
  }

  // 选择用户
  const handleSelectUser = (userId: number, checked: boolean) => {
    if (checked) {
      setSelectedUsers([...selectedUsers, userId])
    } else {
      setSelectedUsers(selectedUsers.filter(id => id !== userId))
    }
  }

  // 全选/取消全选
  const handleSelectAll = (checked: boolean) => {
    if (checked) {
      setSelectedUsers(users.map(user => user.id))
    } else {
      setSelectedUsers([])
    }
  }

  return (
    <div className="w-full">
      {/* 错误提示 */}
      {error && (
        <div className="mb-4 p-4 bg-red-50 border border-red-200 rounded-lg flex items-center gap-2 text-red-700">
          <AlertCircle size={16} />
          <span>{error}</span>
          <button
            onClick={() => setError(null)}
            className="ml-auto text-red-500 hover:text-red-700"
          >
            ×
          </button>
        </div>
      )}

      {/* 页面头部 */}
      <div className="flex items-center justify-between mb-6">
        <div>
          <h2 className="text-lg font-medium text-neutral-800">用户管理</h2>
          <p className="text-sm text-neutral-600 mt-1">管理系统用户和权限设置</p>
        </div>
        <div className="flex gap-2">
          <button
            onClick={() => {
              loadUsers()
              loadStats()
            }}
            disabled={pageLoading}
            className="flex items-center gap-2 px-3 py-2 text-sm text-neutral-600 hover:text-neutral-800 disabled:opacity-50"
            title="刷新数据"
          >
            {pageLoading ? <LoadingSpinner size="sm" tone="muted" /> : <RefreshCw size={16} />}
            刷新
          </button>
          {selectedUsers.length > 0 && (
            <button
              onClick={() => setDialogType('batchDelete')}
              className="flex items-center gap-2 bg-red-600 text-white px-4 py-2 rounded-lg text-sm font-medium hover:bg-red-700"
            >
              <Trash2 size={16} />
              删除选中 ({selectedUsers.length})
            </button>
          )}
          <button
            onClick={() => setDialogType('add')}
            className="flex items-center gap-2 bg-violet-600 text-white px-4 py-2 rounded-lg text-sm font-medium hover:bg-violet-700"
          >
            <Plus size={16} />
            新增用户
          </button>
        </div>
      </div>

      {/* 统计信息 */}
      {pageLoading ? (
        <div className="mb-6 grid grid-cols-1 md:grid-cols-4 gap-4">
          {[1, 2, 3, 4].map((i) => (
            <div
              key={i}
              className="rounded-2xl border border-neutral-200 bg-white p-4 shadow-sm"
            >
              <div className="flex items-center gap-3">
                <Skeleton className="h-10 w-10 rounded-xl" />
                <div className="flex-1">
                  <Skeleton className="h-4 w-20 mb-2" />
                  <Skeleton className="h-6 w-24" />
                </div>
              </div>
            </div>
          ))}
        </div>
      ) : (
        <div className="mb-6 grid grid-cols-1 md:grid-cols-4 gap-4">
          <StatCard
            icon={<Users size={18} />}
            value={stats.total.toString()}
            label="总用户数"
            color="#7c3aed"
          />
          <StatCard
            icon={<Activity size={18} />}
            value={stats.active.toString()}
            label="活跃用户"
            color="#10b981"
          />
          <StatCard
            icon={<Shield size={18} />}
            value={stats.admin.toString()}
            label="管理员"
            color="#ef4444"
          />
          <StatCard
            icon={<BarChart3 size={18} />}
            value={stats.inactive.toString()}
            label="非活跃用户"
            color="#f59e0b"
          />
        </div>
      )}

      {/* 搜索和过滤 */}
      <div className="flex items-center gap-4 mb-4">
        <div className="relative flex-1 max-w-md">
          <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 text-neutral-400" size={16} />
          <input
            type="text"
            placeholder="搜索用户名或邮箱..."
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            className="w-full pl-10 pr-4 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>
        <div className="flex items-center gap-4">
          <FilterSelect
            value={isActiveFilter}
            onValueChange={(value) => setIsActiveFilter(value as 'all' | 'true' | 'false')}
            options={[
              { value: 'all', label: '全部状态' },
              { value: 'true', label: '活跃' },
              { value: 'false', label: '非活跃' }
            ]}
            placeholder="全部状态"
          />
          <FilterSelect
            value={isAdminFilter}
            onValueChange={(value) => setIsAdminFilter(value as 'all' | 'true' | 'false')}
            options={[
              { value: 'all', label: '全部角色' },
              { value: 'true', label: '管理员' },
              { value: 'false', label: '普通用户' }
            ]}
            placeholder="全部角色"
          />
        </div>
      </div>

      {/* 数据表格 */}
      <div className="bg-white rounded-2xl border border-neutral-200 overflow-hidden">
      {pageLoading ? (
        <div className="flex items-center justify-center py-12">
          <LoadingState text="加载中..." />
        </div>
      ) : (
          <>
            <div className="overflow-x-auto">
              <table className="w-full text-sm min-w-[1200px]">
                <thead className="bg-neutral-50 text-neutral-600">
                  <tr>
                    <th className="px-4 py-3 text-left w-[50px]">
                      <input
                        type="checkbox"
                        checked={selectedUsers.length === users.length && users.length > 0}
                        onChange={(e) => handleSelectAll(e.target.checked)}
                        className="rounded border-neutral-300"
                      />
                    </th>
                    <th className="px-4 py-3 text-left font-medium w-[280px]">用户</th>
                    <th className="px-4 py-3 text-left font-medium w-[80px]">角色</th>
                    <th className="px-4 py-3 text-left font-medium w-[80px]">状态</th>
                    <th className="px-4 py-3 text-left font-medium w-[100px]">请求数</th>
                    <th className="px-4 py-3 text-left font-medium w-[100px]">花费</th>
                    <th className="px-4 py-3 text-left font-medium w-[120px]">Token</th>
                    <th className="px-4 py-3 text-left font-medium w-[140px]">最后登录</th>
                    <th className="px-4 py-3 text-left font-medium w-[140px]">创建时间</th>
                    <th className="px-4 py-3 text-left font-medium w-[150px]">操作</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-neutral-200">
                  {users.map((user) => (
                    <tr key={user.id} className="text-neutral-800 hover:bg-neutral-50">
                      <td className="px-4 py-3 w-[50px]">
                        <input
                          type="checkbox"
                          checked={selectedUsers.includes(user.id)}
                          onChange={(e) => handleSelectUser(user.id, e.target.checked)}
                          className="rounded border-neutral-300"
                        />
                      </td>
                      <td className="px-4 py-3 w-[280px]">
                        <div className="flex items-center gap-3">
                          <div className="h-10 w-10 rounded-full bg-violet-100 flex items-center justify-center">
                            <User size={18} className="text-violet-600" />
                          </div>
                          <div className="min-w-0 flex-1">
                            <div className="font-medium truncate">{user.username}</div>
                            <div className="text-xs text-neutral-500 flex items-center gap-1 truncate">
                              <Mail size={10} />
                              <span className="truncate">{user.email}</span>
                            </div>
                          </div>
                        </div>
                      </td>
                      <td className="px-4 py-3 w-[80px]">{renderUserRole(user.is_admin)}</td>
                      <td className="px-4 py-3 w-[80px]">{renderUserStatus(user.is_active)}</td>
                      <td className="px-4 py-3 w-[100px]">
                        <div className="flex items-center gap-1">
                          <BarChart3 size={12} className="text-blue-400" />
                          <span className="text-xs font-medium text-blue-600">
                            {user.total_requests.toLocaleString()}
                          </span>
                        </div>
                      </td>
                      <td className="px-4 py-3 w-[100px]">
                        <div className="flex items-center gap-1">
                          <span className="text-xs text-green-600 font-medium">
                            ${user.total_cost.toFixed(2)}
                          </span>
                        </div>
                      </td>
                      <td className="px-4 py-3 w-[120px]">
                        <div className="flex items-center gap-1">
                          <span className="text-xs text-purple-600 font-medium">
                            {user.total_tokens.toLocaleString()}
                          </span>
                        </div>
                      </td>
                      <td className="px-4 py-3 w-[140px]">
                        <div className="flex items-center gap-1">
                          <Clock size={12} className="text-neutral-400" />
                          <span className="text-xs truncate">{formatLastLogin(user.last_login)}</span>
                        </div>
                      </td>
                      <td className="px-4 py-3 w-[140px]">
                        <div className="flex items-center gap-1">
                          <Calendar size={12} className="text-neutral-400" />
                          <span className="text-xs truncate">{formatDate(user.created_at)}</span>
                        </div>
                      </td>
                      <td className="px-4 py-3 w-[150px]">
                        <div className="flex items-center gap-1">
                          <button
                            onClick={() => handleToggleStatus(user)}
                            className="p-1 text-neutral-500 hover:text-orange-600"
                            title={user.is_active ? '停用用户' : '启用用户'}
                          >
                            {user.is_active ? <ToggleRight size={16} /> : <ToggleLeft size={16} />}
                          </button>
                          <button
                            onClick={() => {
                              setSelectedUser(user)
                              setDialogType('details')
                            }}
                            className="p-1 text-neutral-500 hover:text-blue-600"
                            title="查看详情"
                          >
                            <Eye size={16} />
                          </button>
                          <button
                            onClick={() => {
                              setSelectedUser(user)
                              setDialogType('edit')
                            }}
                            className="p-1 text-neutral-500 hover:text-violet-600"
                            title="编辑"
                          >
                            <Edit size={16} />
                          </button>
                          <button
                            onClick={() => {
                              setSelectedUser(user)
                              setDialogType('resetPassword')
                            }}
                            className="p-1 text-neutral-500 hover:text-green-600"
                            title="重置密码"
                          >
                            <Key size={16} />
                          </button>
                          <button
                            onClick={() => {
                              setSelectedUser(user)
                              setDialogType('delete')
                            }}
                            className="p-1 text-neutral-500 hover:text-red-600"
                            title="删除"
                          >
                            <Trash2 size={16} />
                          </button>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            
            {/* 分页组件 */}
            {totalPages > 1 && (
              <div className="flex items-center justify-between px-4 py-3 border-t border-neutral-200">
                <div className="text-sm text-neutral-600">
                  显示 {(currentPage - 1) * pageSize + 1} - {Math.min(currentPage * pageSize, totalUsers)} 条，共 {totalUsers} 条记录
                </div>
                <div className="flex items-center gap-4">
                  {/* 每页数量选择 */}
                  <div className="flex items-center gap-2">
                    <span className="text-sm text-neutral-600">每页</span>
                    <ModernSelect
                      value={pageSize.toString()}
                      onValueChange={(value) => {
                        const newSize = Number(value)
                        setPageSize(newSize)
                        setCurrentPage(1)
                      }}
                      options={[
                        { value: '10', label: '10' },
                        { value: '20', label: '20' },
                        { value: '50', label: '50' },
                        { value: '100', label: '100' }
                      ]}
                      triggerClassName="h-8 w-16"
                    />
                    <span className="text-sm text-neutral-600">条</span>
                  </div>
                  
                  <div className="flex items-center gap-2">
                    <button
                      onClick={() => setCurrentPage(prev => Math.max(prev - 1, 1))}
                      disabled={currentPage === 1}
                      className={`flex items-center gap-1 px-3 py-1.5 text-sm rounded-lg border ${
                        currentPage === 1
                          ? 'bg-neutral-50 text-neutral-400 border-neutral-200 cursor-not-allowed'
                          : 'bg-white text-neutral-700 border-neutral-200 hover:bg-neutral-50'
                      }`}
                    >
                      <ChevronLeft size={16} />
                      上一页
                    </button>
                    
                    <div className="flex items-center gap-1">
                      {Array.from({ length: Math.min(totalPages, 7) }, (_, i) => {
                        let page
                        if (totalPages <= 7) {
                          page = i + 1
                        } else if (currentPage <= 4) {
                          page = i + 1
                        } else if (currentPage >= totalPages - 3) {
                          page = totalPages - 6 + i
                        } else {
                          page = currentPage - 3 + i
                        }
                        
                        return (
                          <button
                            key={page}
                            onClick={() => setCurrentPage(page)}
                            className={`px-3 py-1.5 text-sm rounded-lg ${
                              page === currentPage
                                ? 'bg-violet-600 text-white'
                                : 'bg-white text-neutral-700 border border-neutral-200 hover:bg-neutral-50'
                            }`}
                          >
                            {page}
                          </button>
                        )
                      })}
                    </div>
                    
                    <button
                      onClick={() => setCurrentPage(prev => Math.min(prev + 1, totalPages))}
                      disabled={currentPage === totalPages}
                      className={`flex items-center gap-1 px-3 py-1.5 text-sm rounded-lg border ${
                        currentPage === totalPages
                          ? 'bg-neutral-50 text-neutral-400 border-neutral-200 cursor-not-allowed'
                          : 'bg-white text-neutral-700 border-neutral-200 hover:bg-neutral-50'
                      }`}
                    >
                      下一页
                      <ChevronRight size={16} />
                    </button>
                  </div>
                </div>
              </div>
            )}
          </>
        )}
      </div>

      {/* 对话框组件 */}
      {dialogType && (
        <UserDialogPortal
          type={dialogType}
          selectedUser={selectedUser}
          selectedUsers={selectedUsers}
          onClose={() => {
            setDialogType(null)
            setSelectedUser(null)
          }}
          onCreateUser={handleCreateUser}
          onUpdateUser={handleUpdateUser}
          onDeleteUser={handleDeleteUser}
          onBatchDelete={handleBatchDelete}
          onResetPassword={handleResetPassword}
        />
      )}
    </div>
  )
}

/** 用户对话框门户组件 */
const UserDialogPortal: React.FC<{
  type: DialogType
  selectedUser: UserType | null
  selectedUsers: number[]
  onClose: () => void
  onCreateUser: (data: CreateUserRequest) => void
  onUpdateUser: (data: UpdateUserRequest) => void
  onDeleteUser: () => void
  onBatchDelete: () => void
  onResetPassword: (password: string) => void
}> = ({ 
  type, 
  selectedUser, 
  selectedUsers,
  onClose, 
  onCreateUser, 
  onUpdateUser, 
  onDeleteUser,
  onBatchDelete,
  onResetPassword 
}) => {
  if (!type) return null

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      {type === 'add' && <AddUserDialog onClose={onClose} onSubmit={onCreateUser} />}
      {type === 'edit' && selectedUser && <EditUserDialog user={selectedUser} onClose={onClose} onSubmit={onUpdateUser} />}
      {type === 'delete' && selectedUser && <DeleteUserDialog user={selectedUser} onClose={onClose} onConfirm={onDeleteUser} />}
      {type === 'batchDelete' && <BatchDeleteDialog count={selectedUsers.length} onClose={onClose} onConfirm={onBatchDelete} />}
      {type === 'details' && selectedUser && <UserDetailsDialog user={selectedUser} onClose={onClose} />}
      {type === 'resetPassword' && selectedUser && <ResetPasswordDialog user={selectedUser} onClose={onClose} onSubmit={onResetPassword} />}
    </div>
  )
}

/** 添加用户对话框 */
const AddUserDialog: React.FC<{
  onClose: () => void
  onSubmit: (data: CreateUserRequest) => void
}> = ({ onClose, onSubmit }) => {
  const [formData, setFormData] = useState<CreateUserRequest>({
    username: '',
    email: '',
    password: '',
    is_admin: false,
  })

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    onSubmit(formData)
  }

  return (
    <div className="bg-white rounded-2xl p-6 w-full max-w-md mx-4">
      <h3 className="text-lg font-medium text-neutral-900 mb-4">新增用户</h3>
      <form onSubmit={handleSubmit} className="space-y-4">
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">
            <span className="text-red-500">*</span> 用户名
          </label>
          <input
            type="text"
            required
            value={formData.username}
            onChange={(e) => setFormData({ ...formData, username: e.target.value })}
            placeholder="请输入用户名 (3-50字符)"
            className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">
            <span className="text-red-500">*</span> 邮箱
          </label>
          <input
            type="email"
            required
            value={formData.email}
            onChange={(e) => setFormData({ ...formData, email: e.target.value })}
            placeholder="请输入邮箱"
            className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">
            <span className="text-red-500">*</span> 密码
          </label>
          <input
            type="password"
            required
            value={formData.password}
            onChange={(e) => setFormData({ ...formData, password: e.target.value })}
            placeholder="请输入密码 (至少8字符)"
            className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>
        <div>
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={formData.is_admin}
              onChange={(e) => setFormData({ ...formData, is_admin: e.target.checked })}
              className="rounded border-neutral-300"
            />
            <span className="text-sm text-neutral-700">设为管理员</span>
          </label>
        </div>
        <div className="flex gap-3 pt-4">
          <button
            type="button"
            onClick={onClose}
            className="flex-1 px-4 py-2 text-sm text-neutral-600 border border-neutral-200 rounded-lg hover:bg-neutral-50"
          >
            取消
          </button>
          <button
            type="submit"
            className="flex-1 px-4 py-2 text-sm bg-violet-600 text-white rounded-lg hover:bg-violet-700"
          >
            创建
          </button>
        </div>
      </form>
    </div>
  )
}

/** 编辑用户对话框 */
const EditUserDialog: React.FC<{
  user: UserType
  onClose: () => void
  onSubmit: (data: UpdateUserRequest) => void
}> = ({ user, onClose, onSubmit }) => {
  const [formData, setFormData] = useState<UpdateUserRequest>({
    username: user.username,
    email: user.email,
    is_active: user.is_active,
    is_admin: user.is_admin,
  })

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    onSubmit(formData)
  }

  return (
    <div className="bg-white rounded-2xl p-6 w-full max-w-md mx-4">
      <h3 className="text-lg font-medium text-neutral-900 mb-4">编辑用户</h3>
      <form onSubmit={handleSubmit} className="space-y-4">
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">用户名</label>
          <input
            type="text"
            value={formData.username || ''}
            onChange={(e) => setFormData({ ...formData, username: e.target.value })}
            placeholder="请输入用户名"
            className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">邮箱</label>
          <input
            type="email"
            value={formData.email || ''}
            onChange={(e) => setFormData({ ...formData, email: e.target.value })}
            placeholder="请输入邮箱"
            className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">新密码 (留空则不修改)</label>
          <input
            type="password"
            value={formData.password || ''}
            onChange={(e) => setFormData({ ...formData, password: e.target.value })}
            placeholder="请输入新密码"
            className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>
        <div className="space-y-2">
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={formData.is_active || false}
              onChange={(e) => setFormData({ ...formData, is_active: e.target.checked })}
              className="rounded border-neutral-300"
            />
            <span className="text-sm text-neutral-700">用户状态活跃</span>
          </label>
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={formData.is_admin || false}
              onChange={(e) => setFormData({ ...formData, is_admin: e.target.checked })}
              className="rounded border-neutral-300"
            />
            <span className="text-sm text-neutral-700">管理员权限</span>
          </label>
        </div>
        <div className="flex gap-3 pt-4">
          <button
            type="button"
            onClick={onClose}
            className="flex-1 px-4 py-2 text-sm text-neutral-600 border border-neutral-200 rounded-lg hover:bg-neutral-50"
          >
            取消
          </button>
          <button
            type="submit"
            className="flex-1 px-4 py-2 text-sm bg-violet-600 text-white rounded-lg hover:bg-violet-700"
          >
            保存
          </button>
        </div>
      </form>
    </div>
  )
}

/** 删除确认对话框 */
const DeleteUserDialog: React.FC<{
  user: UserType
  onClose: () => void
  onConfirm: () => void
}> = ({ user, onClose, onConfirm }) => {
  return (
    <div className="bg-white rounded-2xl p-6 w-full max-w-md mx-4">
      <h3 className="text-lg font-medium text-neutral-900 mb-2">确认删除</h3>
      <p className="text-sm text-neutral-600 mb-4">
        确定要删除用户 <strong>{user.username}</strong> 吗？此操作无法撤销。
      </p>
      <div className="flex gap-3">
        <button
          onClick={onClose}
          className="flex-1 px-4 py-2 text-sm text-neutral-600 border border-neutral-200 rounded-lg hover:bg-neutral-50"
        >
          取消
        </button>
        <button
          onClick={onConfirm}
          className="flex-1 px-4 py-2 text-sm bg-red-600 text-white rounded-lg hover:bg-red-700"
        >
          删除
        </button>
      </div>
    </div>
  )
}

/** 批量删除确认对话框 */
const BatchDeleteDialog: React.FC<{
  count: number
  onClose: () => void
  onConfirm: () => void
}> = ({ count, onClose, onConfirm }) => {
  return (
    <div className="bg-white rounded-2xl p-6 w-full max-w-md mx-4">
      <h3 className="text-lg font-medium text-neutral-900 mb-2">确认批量删除</h3>
      <p className="text-sm text-neutral-600 mb-4">
        确定要删除选中的 <strong>{count}</strong> 个用户吗？此操作无法撤销。
      </p>
      <div className="flex gap-3">
        <button
          onClick={onClose}
          className="flex-1 px-4 py-2 text-sm text-neutral-600 border border-neutral-200 rounded-lg hover:bg-neutral-50"
        >
          取消
        </button>
        <button
          onClick={onConfirm}
          className="flex-1 px-4 py-2 text-sm bg-red-600 text-white rounded-lg hover:bg-red-700"
        >
          删除
        </button>
      </div>
    </div>
  )
}

/** 重置密码对话框 */
const ResetPasswordDialog: React.FC<{
  user: UserType
  onClose: () => void
  onSubmit: (password: string) => void
}> = ({ user, onClose, onSubmit }) => {
  const [newPassword, setNewPassword] = useState('')

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (newPassword.length >= 8) {
      onSubmit(newPassword)
    }
  }

  return (
    <div className="bg-white rounded-2xl p-6 w-full max-w-md mx-4">
      <h3 className="text-lg font-medium text-neutral-900 mb-4">重置密码</h3>
      <p className="text-sm text-neutral-600 mb-4">
        为用户 <strong>{user.username}</strong> 设置新密码
      </p>
      <form onSubmit={handleSubmit} className="space-y-4">
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">
            <span className="text-red-500">*</span> 新密码
          </label>
          <input
            type="password"
            required
            value={newPassword}
            onChange={(e) => setNewPassword(e.target.value)}
            placeholder="请输入新密码 (至少8字符)"
            className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>
        <div className="flex gap-3 pt-4">
          <button
            type="button"
            onClick={onClose}
            className="flex-1 px-4 py-2 text-sm text-neutral-600 border border-neutral-200 rounded-lg hover:bg-neutral-50"
          >
            取消
          </button>
          <button
            type="submit"
            disabled={newPassword.length < 8}
            className="flex-1 px-4 py-2 text-sm bg-green-600 text-white rounded-lg hover:bg-green-700 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            重置密码
          </button>
        </div>
      </form>
    </div>
  )
}

/** 用户详情对话框 */
const UserDetailsDialog: React.FC<{
  user: UserType
  onClose: () => void
}> = ({ user, onClose }) => {
  return (
    <div className="bg-white rounded-2xl p-6 w-full max-w-lg mx-4 max-h-[80vh] overflow-y-auto">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-lg font-medium text-neutral-900">用户详情</h3>
        <button
          onClick={onClose}
          className="text-neutral-500 hover:text-neutral-700"
        >
          ×
        </button>
      </div>
      
      <div className="space-y-4">
        <div className="flex items-center gap-4">
          <div className="h-16 w-16 rounded-full bg-violet-100 flex items-center justify-center">
            <User size={24} className="text-violet-600" />
          </div>
          <div>
            <div className="text-lg font-medium">{user.username}</div>
            <div className="text-sm text-neutral-600">{user.email}</div>
          </div>
        </div>

        <div className="grid grid-cols-2 gap-4">
          <div className="p-3 bg-neutral-50 rounded-lg">
            <div className="text-sm text-neutral-600">用户角色</div>
            <div className="mt-1">{user.is_admin ? '管理员' : '普通用户'}</div>
          </div>
          <div className="p-3 bg-neutral-50 rounded-lg">
            <div className="text-sm text-neutral-600">账户状态</div>
            <div className="mt-1">{user.is_active ? '活跃' : '非活跃'}</div>
          </div>
        </div>

        <div className="grid grid-cols-2 gap-4">
          <div className="p-3 bg-neutral-50 rounded-lg">
            <div className="text-sm text-neutral-600">创建时间</div>
            <div className="mt-1">{new Date(user.created_at).toLocaleString()}</div>
          </div>
          <div className="p-3 bg-neutral-50 rounded-lg">
            <div className="text-sm text-neutral-600">更新时间</div>
            <div className="mt-1">{new Date(user.updated_at).toLocaleString()}</div>
          </div>
        </div>

        <div className="grid grid-cols-3 gap-4">
          <div className="p-3 bg-blue-50 rounded-lg">
            <div className="text-sm text-blue-600">总请求数</div>
            <div className="mt-1 text-blue-900 font-medium">{user.total_requests.toLocaleString()}</div>
          </div>
          <div className="p-3 bg-green-50 rounded-lg">
            <div className="text-sm text-green-600">总花费</div>
            <div className="mt-1 text-green-900 font-medium">${user.total_cost.toFixed(2)}</div>
          </div>
          <div className="p-3 bg-purple-50 rounded-lg">
            <div className="text-sm text-purple-600">总Token数</div>
            <div className="mt-1 text-purple-900 font-medium">{user.total_tokens.toLocaleString()}</div>
          </div>
        </div>

        {user.last_login && (
          <div className="p-3 bg-orange-50 rounded-lg">
            <div className="text-sm text-orange-600">最后登录时间</div>
            <div className="mt-1 text-orange-900">{new Date(user.last_login).toLocaleString()}</div>
          </div>
        )}
      </div>
    </div>
  )
}

export default UsersPage

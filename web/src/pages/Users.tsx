/**
 * Users.tsx
 * 用户管理页：完整的用户数据管理、搜索过滤和分页功能
 */

import React, { useState, useMemo } from 'react'
import {
  Search,
  Plus,
  Edit,
  Trash2,
  Eye,
  Filter,
  RefreshCw,
  User,
  Mail,
  Calendar,
  Shield,
  Activity,
  Clock,
  ChevronLeft,
  ChevronRight,
  Settings,
  Ban,
  CheckCircle,
  Users,
  BarChart3,
} from 'lucide-react'
import { StatCard } from '../components/common/StatCard'
import FilterSelect from '../components/common/FilterSelect'
import ModernSelect from '../components/common/ModernSelect'

/** 用户角色类型 */
type UserRole = 'admin' | 'user' | 'moderator'

/** 用户状态类型 */
type UserStatus = 'active' | 'inactive' | 'banned'

/** 用户数据结构 */
interface UserData {
  id: string
  username: string
  password: string
  email?: string
  avatar: string
  role: UserRole
  status: UserStatus
  lastLogin: string
  joinDate: string
  requestCount: number
  monthlyRequests: number
  totalCost: number
}

/** 模拟用户数据 */
const generateMockUsers = (): UserData[] => {
  const avatars = [
    'https://pub-cdn.sider.ai/u/U024HX2V46R/web-coder/689c58b6f5303283889f5c38/resource/e030b28b-4ccb-44fd-8fda-29acbd37cde2.jpg',
    'https://pub-cdn.sider.ai/u/U024HX2V46R/web-coder/689c58b6f5303283889f5c38/resource/06fb1510-bf11-46d5-b986-fe2e72b98b34.jpg'
  ]
  
  const usernames = ['admin', 'zhangsan', 'lisi', 'wangwu', 'alice', 'bob', 'charlie', 'david', 'emma', 'frank', 'user001', 'user002', 'user003', 'moderator01']
  const roles: UserRole[] = ['admin', 'user', 'moderator']
  const statuses: UserStatus[] = ['active', 'inactive', 'banned']
  
  const users: UserData[] = []
  for (let i = 0; i < 50; i++) {
    const username = usernames[Math.floor(Math.random() * usernames.length)] + (i > 9 ? i : '')
    const role = roles[Math.floor(Math.random() * roles.length)]
    const status = statuses[Math.floor(Math.random() * statuses.length)]
    
    // 管理员更可能是活跃状态
    const finalStatus = role === 'admin' ? 'active' : status
    
    const joinDate = new Date()
    joinDate.setDate(joinDate.getDate() - Math.floor(Math.random() * 365))
    
    const lastLogin = new Date()
    lastLogin.setHours(lastLogin.getHours() - Math.floor(Math.random() * 168))
    
    users.push({
      id: `user-${i + 1}`,
      username: username,
      password: 'password123', // 演示密码
      email: Math.random() > 0.3 ? `${username}@example.com` : undefined, // 70%用户有邮箱
      avatar: avatars[i % avatars.length],
      role,
      status: finalStatus,
      lastLogin: lastLogin.toISOString(),
      joinDate: joinDate.toISOString().split('T')[0],
      requestCount: Math.floor(Math.random() * 10000),
      monthlyRequests: Math.floor(Math.random() * 1000),
      totalCost: Math.random() * 500
    })
  }
  
  return users.sort((a, b) => new Date(b.lastLogin).getTime() - new Date(a.lastLogin).getTime())
}

const initialData = generateMockUsers()

/** 弹窗类型 */
type DialogType = 'add' | 'edit' | 'delete' | 'details' | null

/** 页面主组件 */
const UsersPage: React.FC = () => {
  const [data, setData] = useState<UserData[]>(initialData)
  const [searchTerm, setSearchTerm] = useState('')
  const [roleFilter, setRoleFilter] = useState<'all' | UserRole>('all')
  const [statusFilter, setStatusFilter] = useState<'all' | UserStatus>('all')
  const [selectedItem, setSelectedItem] = useState<UserData | null>(null)
  const [dialogType, setDialogType] = useState<DialogType>(null)
  
  // 分页状态
  const [currentPage, setCurrentPage] = useState(1)
  const [pageSize, setPageSize] = useState(15)

  // 过滤数据
  const filteredData = useMemo(() => {
    return data.filter((item) => {
      const matchesSearch = 
        item.username.toLowerCase().includes(searchTerm.toLowerCase()) ||
        (item.email && item.email.toLowerCase().includes(searchTerm.toLowerCase()))
      const matchesRole = roleFilter === 'all' || item.role === roleFilter
      const matchesStatus = statusFilter === 'all' || item.status === statusFilter
      return matchesSearch && matchesRole && matchesStatus
    })
  }, [data, searchTerm, roleFilter, statusFilter])

  // 分页数据和计算
  const paginatedData = useMemo(() => {
    const startIndex = (currentPage - 1) * pageSize
    return filteredData.slice(startIndex, startIndex + pageSize)
  }, [filteredData, currentPage, pageSize])

  const totalPages = Math.ceil(filteredData.length / pageSize)
  
  // 重置页码当过滤条件改变时
  React.useEffect(() => {
    setCurrentPage(1)
  }, [searchTerm, roleFilter, statusFilter])

  // 格式化时间
  const formatLastLogin = (timestamp: string) => {
    const date = new Date(timestamp)
    const now = new Date()
    const diffInHours = Math.floor((now.getTime() - date.getTime()) / (1000 * 60 * 60))
    
    if (diffInHours < 1) return '刚刚'
    if (diffInHours < 24) return `${diffInHours}小时前`
    if (diffInHours < 168) return `${Math.floor(diffInHours / 24)}天前`
    return date.toLocaleDateString()
  }

  // 渲染用户角色
  const renderUserRole = (role: UserRole) => {
    const roleConfig = {
      admin: { color: 'text-red-600', bg: 'bg-red-50', ring: 'ring-red-200', text: '管理员' },
      moderator: { color: 'text-blue-600', bg: 'bg-blue-50', ring: 'ring-blue-200', text: '协调员' },
      user: { color: 'text-neutral-600', bg: 'bg-neutral-50', ring: 'ring-neutral-200', text: '普通用户' },
    }
    const config = roleConfig[role]
    
    return (
      <span className={`inline-flex items-center px-2 py-1 rounded-full text-xs font-medium ${config.bg} ${config.color} ring-1 ${config.ring}`}>
        <Shield size={10} className="mr-1" />
        {config.text}
      </span>
    )
  }

  // 渲染用户状态
  const renderUserStatus = (status: UserStatus) => {
    const statusConfig = {
      active: { color: 'text-emerald-600', bg: 'bg-emerald-50', ring: 'ring-emerald-200', text: '活跃' },
      inactive: { color: 'text-yellow-600', bg: 'bg-yellow-50', ring: 'ring-yellow-200', text: '非活跃' },
      banned: { color: 'text-red-600', bg: 'bg-red-50', ring: 'ring-red-200', text: '已封禁' },
    }
    const config = statusConfig[status]
    
    return (
      <span className={`inline-flex items-center px-2 py-1 rounded-full text-xs font-medium ${config.bg} ${config.color} ring-1 ${config.ring}`}>
        <Activity size={10} className="mr-1" />
        {config.text}
      </span>
    )
  }

  // 添加用户
  const handleAdd = (newUser: Omit<UserData, 'id' | 'requestCount' | 'monthlyRequests' | 'totalCost'>) => {
    const user: UserData = {
      ...newUser,
      id: Date.now().toString(),
      requestCount: 0,
      monthlyRequests: 0,
      totalCost: 0,
    }
    setData([user, ...data])
    setDialogType(null)
  }

  // 编辑用户
  const handleEdit = (updatedUser: UserData) => {
    setData(data.map(item => item.id === updatedUser.id ? updatedUser : item))
    setDialogType(null)
    setSelectedItem(null)
  }

  // 删除用户
  const handleDelete = () => {
    if (selectedItem) {
      setData(data.filter(item => item.id !== selectedItem.id))
      setDialogType(null)
      setSelectedItem(null)
    }
  }

  return (
    <div className="w-full">
      {/* 页面头部 */}
      <div className="flex items-center justify-between mb-6">
        <div>
          <h2 className="text-lg font-medium text-neutral-800">用户管理</h2>
          <p className="text-sm text-neutral-600 mt-1">管理系统用户和权限设置</p>
        </div>
        <div className="flex gap-2">
          <button
            onClick={() => setData([...initialData])}
            className="flex items-center gap-2 px-3 py-2 text-sm text-neutral-600 hover:text-neutral-800"
            title="刷新数据"
          >
            <RefreshCw size={16} />
            刷新
          </button>
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
      <div className="mb-6 grid grid-cols-1 md:grid-cols-4 gap-4">
        <StatCard
          icon={<Users size={18} />}
          value={data.length.toString()}
          label="总用户数"
          color="#7c3aed"
        />
        <StatCard
          icon={<Activity size={18} />}
          value={data.filter(item => item.status === 'active').length.toString()}
          label="活跃用户"
          color="#10b981"
        />
        <StatCard
          icon={<Shield size={18} />}
          value={data.filter(item => item.role === 'admin').length.toString()}
          label="管理员"
          color="#ef4444"
        />
        <StatCard
          icon={<BarChart3 size={18} />}
          value={data.reduce((sum, item) => sum + item.requestCount, 0).toLocaleString()}
          label="总请求数"
          color="#0ea5e9"
        />
      </div>

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
            value={roleFilter}
            onValueChange={(value) => setRoleFilter(value as 'all' | UserRole)}
            options={[
              { value: 'all', label: '全部角色' },
              { value: 'admin', label: '管理员' },
              { value: 'moderator', label: '协调员' },
              { value: 'user', label: '普通用户' }
            ]}
            placeholder="全部角色"
          />
          <FilterSelect
            value={statusFilter}
            onValueChange={(value) => setStatusFilter(value as 'all' | UserStatus)}
            options={[
              { value: 'all', label: '全部状态' },
              { value: 'active', label: '活跃' },
              { value: 'inactive', label: '非活跃' },
              { value: 'banned', label: '已封禁' }
            ]}
            placeholder="全部状态"
          />
        </div>
      </div>

      {/* 数据表格 */}
      <div className="bg-white rounded-2xl border border-neutral-200 overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead className="bg-neutral-50 text-neutral-600">
              <tr>
                <th className="px-4 py-3 text-left font-medium">用户</th>
                <th className="px-4 py-3 text-left font-medium">角色</th>
                <th className="px-4 py-3 text-left font-medium">状态</th>
                <th className="px-4 py-3 text-left font-medium">最后登录</th>
                <th className="px-4 py-3 text-left font-medium">使用情况</th>
                <th className="px-4 py-3 text-left font-medium">花费</th>
                <th className="px-4 py-3 text-left font-medium">操作</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-neutral-200">
              {paginatedData.map((item) => (
                <tr key={item.id} className="text-neutral-800 hover:bg-neutral-50">
                  <td className="px-4 py-3">
                    <div className="flex items-center gap-3">
                      <img
                        src={item.avatar}
                        alt={item.username}
                        className="h-10 w-10 rounded-full object-cover ring-2 ring-neutral-200"
                      />
                      <div>
                        <div className="font-medium">{item.username}</div>
                        <div className="text-xs text-neutral-500">{item.email || '未设置邮箱'}</div>
                        <div className="text-xs text-neutral-400 flex items-center gap-1 mt-1">
                          <Calendar size={10} />
                          加入于 {item.joinDate}
                        </div>
                      </div>
                    </div>
                  </td>
                  <td className="px-4 py-3">{renderUserRole(item.role)}</td>
                  <td className="px-4 py-3">{renderUserStatus(item.status)}</td>
                  <td className="px-4 py-3">
                    <div className="flex items-center gap-1">
                      <Clock size={12} className="text-neutral-400" />
                      <span className="text-xs">{formatLastLogin(item.lastLogin)}</span>
                    </div>
                  </td>
                  <td className="px-4 py-3">
                    <div>
                      <div className="text-sm font-medium">{item.requestCount.toLocaleString()}</div>
                      <div className="text-xs text-neutral-500">本月 {item.monthlyRequests}</div>
                    </div>
                  </td>
                  <td className="px-4 py-3">
                    <div className="text-sm font-medium">${item.totalCost.toFixed(2)}</div>
                  </td>
                  <td className="px-4 py-3">
                    <div className="flex items-center gap-1">
                      <button
                        onClick={() => {
                          setSelectedItem(item)
                          setDialogType('details')
                        }}
                        className="p-1 text-neutral-500 hover:text-blue-600"
                        title="查看详情"
                      >
                        <Eye size={16} />
                      </button>
                      <button
                        onClick={() => {
                          setSelectedItem(item)
                          setDialogType('edit')
                        }}
                        className="p-1 text-neutral-500 hover:text-violet-600"
                        title="编辑"
                      >
                        <Edit size={16} />
                      </button>
                      <button
                        onClick={() => {
                          setSelectedItem(item)
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
              显示 {(currentPage - 1) * pageSize + 1} - {Math.min(currentPage * pageSize, filteredData.length)} 条，共 {filteredData.length} 条记录
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
                    setCurrentPage(1) // 重置到第一页
                  }}
                  options={[
                    { value: '10', label: '10' },
                    { value: '15', label: '15' },
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
      </div>


      {/* 对话框组件 */}
      {dialogType && (
        <UserDialogPortal
          type={dialogType}
          selectedItem={selectedItem}
          onClose={() => {
            setDialogType(null)
            setSelectedItem(null)
          }}
          onAdd={handleAdd}
          onEdit={handleEdit}
          onDelete={handleDelete}
        />
      )}
    </div>
  )
}

/** 用户对话框门户组件 */
const UserDialogPortal: React.FC<{
  type: DialogType
  selectedItem: UserData | null
  onClose: () => void
  onAdd: (item: Omit<UserData, 'id' | 'requestCount' | 'monthlyRequests' | 'totalCost'>) => void
  onEdit: (item: UserData) => void
  onDelete: () => void
}> = ({ type, selectedItem, onClose, onAdd, onEdit, onDelete }) => {
  if (!type) return null

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      {type === 'add' && <AddUserDialog onClose={onClose} onSubmit={onAdd} />}
      {type === 'edit' && selectedItem && <EditUserDialog item={selectedItem} onClose={onClose} onSubmit={onEdit} />}
      {type === 'delete' && selectedItem && <DeleteUserDialog item={selectedItem} onClose={onClose} onConfirm={onDelete} />}
      {type === 'details' && selectedItem && <UserDetailsDialog item={selectedItem} onClose={onClose} />}
    </div>
  )
}

/** 添加用户对话框 */
const AddUserDialog: React.FC<{
  onClose: () => void
  onSubmit: (item: Omit<UserData, 'id' | 'requestCount' | 'monthlyRequests' | 'totalCost'>) => void
}> = ({ onClose, onSubmit }) => {
  const [formData, setFormData] = useState({
    username: '',
    password: '',
    email: '',
    avatar: 'https://pub-cdn.sider.ai/u/U024HX2V46R/web-coder/689c58b6f5303283889f5c38/resource/e030b28b-4ccb-44fd-8fda-29acbd37cde2.jpg',
    role: 'user' as UserRole,
    status: 'active' as UserStatus,
    lastLogin: new Date().toISOString(),
    joinDate: new Date().toISOString().split('T')[0],
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
            placeholder="请输入用户名"
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
            placeholder="请输入密码"
            className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">邮箱（可选）</label>
          <input
            type="email"
            value={formData.email}
            onChange={(e) => setFormData({ ...formData, email: e.target.value })}
            placeholder="请输入邮箱"
            className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">角色</label>
          <ModernSelect
            value={formData.role}
            onValueChange={(value) => setFormData({ ...formData, role: value as UserRole })}
            options={[
              { value: 'user', label: '普通用户' },
              { value: 'moderator', label: '协调员' },
              { value: 'admin', label: '管理员' }
            ]}
            placeholder="请选择角色"
          />
        </div>
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">状态</label>
          <ModernSelect
            value={formData.status}
            onValueChange={(value) => setFormData({ ...formData, status: value as UserStatus })}
            options={[
              { value: 'active', label: '活跃' },
              { value: 'inactive', label: '非活跃' }
            ]}
            placeholder="请选择状态"
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
  item: UserData
  onClose: () => void
  onSubmit: (item: UserData) => void
}> = ({ item, onClose, onSubmit }) => {
  const [formData, setFormData] = useState({ ...item })

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    onSubmit(formData)
  }

  return (
    <div className="bg-white rounded-2xl p-6 w-full max-w-md mx-4">
      <h3 className="text-lg font-medium text-neutral-900 mb-4">编辑用户</h3>
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
            placeholder="请输入用户名"
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
            placeholder="请输入密码"
            className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">邮箱（可选）</label>
          <input
            type="email"
            value={formData.email || ''}
            onChange={(e) => setFormData({ ...formData, email: e.target.value || undefined })}
            placeholder="请输入邮箱"
            className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">角色</label>
          <ModernSelect
            value={formData.role}
            onValueChange={(value) => setFormData({ ...formData, role: value as UserRole })}
            options={[
              { value: 'user', label: '普通用户' },
              { value: 'moderator', label: '协调员' },
              { value: 'admin', label: '管理员' }
            ]}
            placeholder="请选择角色"
          />
        </div>
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">状态</label>
          <ModernSelect
            value={formData.status}
            onValueChange={(value) => setFormData({ ...formData, status: value as UserStatus })}
            options={[
              { value: 'active', label: '活跃' },
              { value: 'inactive', label: '非活跃' },
              { value: 'banned', label: '已封禁' }
            ]}
            placeholder="请选择状态"
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
  item: UserData
  onClose: () => void
  onConfirm: () => void
}> = ({ item, onClose, onConfirm }) => {
  return (
    <div className="bg-white rounded-2xl p-6 w-full max-w-md mx-4">
      <h3 className="text-lg font-medium text-neutral-900 mb-2">确认删除</h3>
      <p className="text-sm text-neutral-600 mb-4">
        确定要删除用户 <strong>{item.username}</strong> 吗？此操作无法撤销。
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

/** 用户详情对话框 */
const UserDetailsDialog: React.FC<{
  item: UserData
  onClose: () => void
}> = ({ item, onClose }) => {
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
          <img
            src={item.avatar}
            alt={item.username}
            className="h-16 w-16 rounded-full object-cover ring-2 ring-neutral-200"
          />
          <div>
            <div className="text-lg font-medium">{item.username}</div>
            <div className="text-sm text-neutral-600">{item.email || '未设置邮箱'}</div>
          </div>
        </div>

        <div className="grid grid-cols-2 gap-4">
          <div className="p-3 bg-neutral-50 rounded-lg">
            <div className="text-sm text-neutral-600">用户角色</div>
            <div className="mt-1">{item.role === 'admin' ? '管理员' : item.role === 'moderator' ? '协调员' : '普通用户'}</div>
          </div>
          <div className="p-3 bg-neutral-50 rounded-lg">
            <div className="text-sm text-neutral-600">账户状态</div>
            <div className="mt-1">{item.status === 'active' ? '活跃' : item.status === 'inactive' ? '非活跃' : '已封禁'}</div>
          </div>
        </div>

        <div className="grid grid-cols-2 gap-4">
          <div className="p-3 bg-neutral-50 rounded-lg">
            <div className="text-sm text-neutral-600">加入时间</div>
            <div className="mt-1">{item.joinDate}</div>
          </div>
          <div className="p-3 bg-neutral-50 rounded-lg">
            <div className="text-sm text-neutral-600">最后登录</div>
            <div className="mt-1">{new Date(item.lastLogin).toLocaleString()}</div>
          </div>
        </div>

        <div className="grid grid-cols-3 gap-4">
          <div className="p-3 bg-violet-50 rounded-lg">
            <div className="text-sm text-violet-600">总请求数</div>
            <div className="text-lg font-bold text-violet-900">{item.requestCount.toLocaleString()}</div>
          </div>
          <div className="p-3 bg-blue-50 rounded-lg">
            <div className="text-sm text-blue-600">本月请求</div>
            <div className="text-lg font-bold text-blue-900">{item.monthlyRequests}</div>
          </div>
          <div className="p-3 bg-orange-50 rounded-lg">
            <div className="text-sm text-orange-600">总花费</div>
            <div className="text-lg font-bold text-orange-900">${item.totalCost.toFixed(2)}</div>
          </div>
        </div>
      </div>
    </div>
  )
}

export default UsersPage
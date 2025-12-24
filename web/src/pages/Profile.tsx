/**
 * Profile.tsx
 * 个人信息页：用户资料管理和设置
 */

import React, { useState, useEffect } from 'react'
import { useAuthStore } from '../store/auth'
import { api } from '../lib/api'
import { toast } from 'sonner'
import {
  User,
  Mail,
  Calendar,
  Shield,
  Edit,
  Save,
  X,
  BarChart3,
  Clock,
} from 'lucide-react'
import { StatCard } from '../components/common/StatCard'

/** 用户信息类型 */
interface UserProfile {
  name: string
  email: string
  avatar: string
  role: string
  created_at: string
  last_login?: string
  total_requests: number
  monthly_requests: number
}

/** 页面主组件 */
const ProfilePage: React.FC = () => {
  const logout = useAuthStore((s) => s.logout)

  // 用户数据状态
  const [userProfile, setUserProfile] = useState<UserProfile | null>(null)
  const [isLoading, setIsLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const [isEditing, setIsEditing] = useState(false)
  const [editForm, setEditForm] = useState({
    name: '',
    email: ''
  })

  // 加载用户数据
  useEffect(() => {
    loadUserProfile()
  }, [])

  const loadUserProfile = async () => {
    try {
      setIsLoading(true)
      setError(null)

      const response = await api.users.getProfile()

      if (response.success && response.data) {
        const profile = response.data
        setUserProfile(profile)
        setEditForm({
          name: profile.name,
          email: profile.email
        })
      } else {
        setError(response.error?.message || '获取用户档案失败')
        toast.error('获取用户档案失败')
      }
    } catch (err) {
      console.error('Load user profile error:', err)
      setError('网络错误，请稍后重试')
      toast.error('网络错误，请稍后重试')
    } finally {
      setIsLoading(false)
    }
  }

  // 保存用户信息
  const handleSaveProfile = async () => {
    try {
      const response = await api.users.updateProfile({
        email: editForm.email
      })

      if (response.success && response.data) {
        setUserProfile(response.data)
        setIsEditing(false)
        toast.success('用户档案更新成功')
      } else {
        toast.error(response.error?.message || '更新用户档案失败')
      }
    } catch (err) {
      console.error('Update profile error:', err)
      toast.error('网络错误，请稍后重试')
    }
  }

  // 取消编辑
  const handleCancelEdit = () => {
    if (userProfile) {
      setEditForm({
        name: userProfile.name,
        email: userProfile.email
      })
    }
    setIsEditing(false)
  }

  // 加载状态显示
  if (isLoading) {
    return (
      <div className="w-full">
        <div className="flex items-center justify-center h-64">
          <div className="text-center">
            <div className="inline-block animate-spin rounded-full h-8 w-8 border-b-2 border-violet-600"></div>
            <p className="mt-2 text-sm text-neutral-600">加载用户档案中...</p>
          </div>
        </div>
      </div>
    )
  }

  // 错误状态显示
  if (error) {
    return (
      <div className="w-full">
        <div className="flex items-center justify-center h-64">
          <div className="text-center">
            <div className="text-red-500 mb-2">❌</div>
            <p className="text-sm text-neutral-600 mb-4">{error}</p>
            <button
              onClick={() => loadUserProfile()}
              className="px-4 py-2 bg-violet-600 text-white rounded-lg text-sm font-medium hover:bg-violet-700"
            >
              重试
            </button>
          </div>
        </div>
      </div>
    )
  }

  // 确保userProfile不为null
  if (!userProfile) {
    return null
  }

  return (
    <div className="w-full">
      {/* 页面头部 */}
      <div className="flex items-center justify-between mb-6">
        <div>
          <h2 className="text-lg font-medium text-neutral-800">个人资料</h2>
          <p className="text-sm text-neutral-600 mt-1">管理您的账户信息和偏好设置</p>
        </div>
        <div className="flex gap-2">
          {!isEditing ? (
            <button
              onClick={() => setIsEditing(true)}
              className="flex items-center gap-2 px-3 py-2 text-sm text-neutral-600 hover:text-neutral-800"
              title="编辑资料"
            >
              <Edit size={16} />
              编辑资料
            </button>
          ) : (
            <div className="flex gap-2">
              <button
                onClick={handleCancelEdit}
                className="flex items-center gap-2 px-3 py-2 text-sm text-neutral-600 hover:text-neutral-800"
                title="取消"
              >
                <X size={16} />
                取消
              </button>
              <button
                onClick={() => handleSaveProfile()}
                className="flex items-center gap-2 bg-violet-600 text-white px-4 py-2 rounded-lg text-sm font-medium hover:bg-violet-700"
                title="保存"
              >
                <Save size={16} />
                保存
              </button>
            </div>
          )}
        </div>
      </div>

      {/* 使用统计 */}
      <div className="mb-6 grid grid-cols-1 md:grid-cols-3 gap-4">
        <StatCard
          icon={<BarChart3 size={18} />}
          value={userProfile.total_requests.toLocaleString()}
          label="总请求数"
          color="#7c3aed"
        />
        <StatCard
          icon={<Calendar size={18} />}
          value={userProfile.monthly_requests.toLocaleString()}
          label="本月请求"
          color="#0ea5e9"
        />
        <StatCard
          icon={<Clock size={18} />}
          value={userProfile.last_login || '从未登录'}
          label="最后登录"
          color="#10b981"
        />
      </div>

      {/* 用户基本信息 */}
      <div className="bg-white rounded-2xl border border-neutral-200 overflow-hidden mb-6">
        <div className="px-6 py-4 border-b border-neutral-200">
          <h3 className="text-sm font-medium text-neutral-900">基本信息</h3>
        </div>
        <div className="p-6">
          <div className="flex items-start gap-6">
            {/* 头像 */}
            <div className="flex-shrink-0">
              <img
                src={userProfile.avatar}
                alt="用户头像"
                className="h-20 w-20 rounded-full object-cover ring-2 ring-neutral-200"
              />
            </div>

            {/* 用户信息 */}
            <div className="flex-1 grid grid-cols-1 md:grid-cols-2 gap-4">
              <div>
                <label className="block text-sm font-medium text-neutral-700 mb-1">姓名</label>
                {isEditing ? (
                  <input
                    type="text"
                    value={editForm.name}
                    onChange={(e) => setEditForm(prev => ({ ...prev, name: e.target.value }))}
                    className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
                  />
                ) : (
                  <div className="flex items-center gap-2">
                    <User size={16} className="text-neutral-400" />
                    <span className="text-sm text-neutral-900">{userProfile.name}</span>
                  </div>
                )}
              </div>

              <div>
                <label className="block text-sm font-medium text-neutral-700 mb-1">邮箱</label>
                {isEditing ? (
                  <input
                    type="email"
                    value={editForm.email}
                    onChange={(e) => setEditForm(prev => ({ ...prev, email: e.target.value }))}
                    className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
                  />
                ) : (
                  <div className="flex items-center gap-2">
                    <Mail size={16} className="text-neutral-400" />
                    <span className="text-sm text-neutral-900">{userProfile.email}</span>
                  </div>
                )}
              </div>

              <div>
                <label className="block text-sm font-medium text-neutral-700 mb-1">角色</label>
                <div className="flex items-center gap-2">
                  <Shield size={16} className="text-neutral-400" />
                  <span className="inline-flex items-center px-2 py-1 rounded-full text-xs font-medium bg-violet-50 text-violet-700 ring-1 ring-violet-200">
                    {userProfile.role}
                  </span>
                </div>
              </div>

              <div>
                <label className="block text-sm font-medium text-neutral-700 mb-1">注册时间</label>
                <div className="flex items-center gap-2">
                  <Calendar size={16} className="text-neutral-400" />
                  <span className="text-sm text-neutral-600">{userProfile.created_at}</span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>


      {/* 账户操作 */}
      <div className="bg-white rounded-2xl border border-neutral-200 overflow-hidden">
        <div className="px-6 py-4 border-b border-neutral-200">
          <h3 className="text-sm font-medium text-neutral-900">账户操作</h3>
        </div>
        <div className="p-6">
          <div className="flex items-center justify-between">
            <div>
              <div className="text-sm font-medium text-neutral-900">退出登录</div>
              <div className="text-xs text-neutral-600 mt-1">清除本地登录状态并返回登录页面</div>
            </div>
            <button
              onClick={() => logout()}
              className="px-4 py-2 bg-red-600 text-white rounded-lg text-sm font-medium hover:bg-red-700"
            >
              退出登录
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}

export default ProfilePage

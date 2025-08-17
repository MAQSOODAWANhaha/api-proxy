/**
 * Profile.tsx
 * 个人信息页：用户资料管理和设置
 */

import React, { useState } from 'react'
import { useAuthStore } from '../store/auth'
import {
  User,
  Mail,
  Calendar,
  Settings,
  Shield,
  Bell,
  Palette,
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
  joinDate: string
  lastLogin: string
  totalRequests: number
  monthlyRequests: number
}

/** 用户设置类型 */
interface UserSettings {
  emailNotifications: boolean
  securityAlerts: boolean
  themePreference: 'light' | 'dark' | 'auto'
  language: 'zh' | 'en'
}

/** 页面主组件 */
const ProfilePage: React.FC = () => {
  const logout = useAuthStore((s) => s.logout)
  
  // 模拟用户数据
  const [userProfile, setUserProfile] = useState<UserProfile>({
    name: 'Admin',
    email: 'admin@example.com',
    avatar: 'https://pub-cdn.sider.ai/u/U024HX2V46R/web-coder/689c58b6f5303283889f5c38/resource/06fb1510-bf11-46d5-b986-fe2e72b98b34.jpg',
    role: '系统管理员',
    joinDate: '2024-01-01',
    lastLogin: '2024-01-16 15:32',
    totalRequests: 12845,
    monthlyRequests: 2156
  })

  const [userSettings, setUserSettings] = useState<UserSettings>({
    emailNotifications: true,
    securityAlerts: true,
    themePreference: 'light',
    language: 'zh'
  })

  const [isEditing, setIsEditing] = useState(false)
  const [editForm, setEditForm] = useState({
    name: userProfile.name,
    email: userProfile.email
  })

  // 保存用户信息
  const handleSaveProfile = () => {
    setUserProfile(prev => ({
      ...prev,
      name: editForm.name,
      email: editForm.email
    }))
    setIsEditing(false)
  }

  // 取消编辑
  const handleCancelEdit = () => {
    setEditForm({
      name: userProfile.name,
      email: userProfile.email
    })
    setIsEditing(false)
  }

  // 更新设置
  const handleSettingChange = (key: keyof UserSettings, value: any) => {
    setUserSettings(prev => ({
      ...prev,
      [key]: value
    }))
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
                onClick={handleSaveProfile}
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
          value={userProfile.totalRequests.toLocaleString()}
          label="总请求数"
          color="#7c3aed"
        />
        <StatCard
          icon={<Calendar size={18} />}
          value={userProfile.monthlyRequests.toLocaleString()}
          label="本月请求"
          color="#0ea5e9"
        />
        <StatCard
          icon={<Clock size={18} />}
          value={userProfile.lastLogin}
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
                  <span className="text-sm text-neutral-600">{userProfile.joinDate}</span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>


      {/* 系统设置 */}
      <div className="bg-white rounded-2xl border border-neutral-200 overflow-hidden mb-6">
        <div className="px-6 py-4 border-b border-neutral-200">
          <div className="flex items-center gap-2">
            <Settings size={16} className="text-neutral-500" />
            <h3 className="text-sm font-medium text-neutral-900">系统设置</h3>
          </div>
        </div>
        <div className="p-6 space-y-4">
          {/* 通知设置 */}
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <Bell size={16} className="text-neutral-400" />
              <div>
                <div className="text-sm font-medium text-neutral-900">邮件通知</div>
                <div className="text-xs text-neutral-600">接收系统邮件通知</div>
              </div>
            </div>
            <label className="relative inline-flex items-center cursor-pointer">
              <input
                type="checkbox"
                checked={userSettings.emailNotifications}
                onChange={(e) => handleSettingChange('emailNotifications', e.target.checked)}
                className="sr-only peer"
              />
              <div className="w-11 h-6 bg-neutral-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-violet-300 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-neutral-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-violet-600"></div>
            </label>
          </div>

          {/* 安全警报 */}
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <Shield size={16} className="text-neutral-400" />
              <div>
                <div className="text-sm font-medium text-neutral-900">安全警报</div>
                <div className="text-xs text-neutral-600">账户安全相关警报</div>
              </div>
            </div>
            <label className="relative inline-flex items-center cursor-pointer">
              <input
                type="checkbox"
                checked={userSettings.securityAlerts}
                onChange={(e) => handleSettingChange('securityAlerts', e.target.checked)}
                className="sr-only peer"
              />
              <div className="w-11 h-6 bg-neutral-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-violet-300 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-neutral-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-violet-600"></div>
            </label>
          </div>

          {/* 主题设置 */}
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <Palette size={16} className="text-neutral-400" />
              <div>
                <div className="text-sm font-medium text-neutral-900">主题偏好</div>
                <div className="text-xs text-neutral-600">选择界面主题</div>
              </div>
            </div>
            <select
              value={userSettings.themePreference}
              onChange={(e) => handleSettingChange('themePreference', e.target.value)}
              className="border border-neutral-200 rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
            >
              <option value="light">浅色</option>
              <option value="dark">深色</option>
              <option value="auto">跟随系统</option>
            </select>
          </div>

          {/* 语言设置 */}
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <Settings size={16} className="text-neutral-400" />
              <div>
                <div className="text-sm font-medium text-neutral-900">界面语言</div>
                <div className="text-xs text-neutral-600">选择界面显示语言</div>
              </div>
            </div>
            <select
              value={userSettings.language}
              onChange={(e) => handleSettingChange('language', e.target.value)}
              className="border border-neutral-200 rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
            >
              <option value="zh">中文</option>
              <option value="en">English</option>
            </select>
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
              onClick={logout}
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
/**
 * Settings.tsx
 * 系统设置页：完整的系统配置管理
 */

import React, { useState } from 'react'
import ModernSelect from '../components/common/ModernSelect'
import {
  Settings,
  Globe,
  Palette,
  Shield,
  Bell,
  Database,
  Mail,
  Key,
  Save,
  RefreshCw,
  Server,
  Zap,
  Lock,
} from 'lucide-react'

/** 系统设置数据结构 */
interface SystemSettings {
  // 基本设置
  siteName: string
  siteDescription: string
  language: 'zh' | 'en'
  timezone: string
  
  // 主题设置
  defaultTheme: 'light' | 'dark' | 'auto'
  primaryColor: string
  allowUserTheme: boolean
  
  // 安全设置
  sessionTimeout: number
  maxLoginAttempts: number
  enableTwoFactor: boolean
  passwordMinLength: number
  
  // 通知设置
  emailNotifications: boolean
  systemAlerts: boolean
  maintenanceMode: boolean
  
  // API设置
  rateLimitPerMinute: number
  maxRequestSize: number
  enableCaching: boolean
  cacheExpiry: number
  
  // 数据库设置
  backupFrequency: 'daily' | 'weekly' | 'monthly'
  retentionDays: number
  enableAutoCleanup: boolean
}

/** 页面主组件 */
const SettingsPage: React.FC = () => {
  const [settings, setSettings] = useState<SystemSettings>({
    siteName: 'AI Proxy Console',
    siteDescription: '智能代理服务管理控制台',
    language: 'zh',
    timezone: 'Asia/Shanghai',
    defaultTheme: 'light',
    primaryColor: '#7c3aed',
    allowUserTheme: true,
    sessionTimeout: 24,
    maxLoginAttempts: 5,
    enableTwoFactor: false,
    passwordMinLength: 8,
    emailNotifications: true,
    systemAlerts: true,
    maintenanceMode: false,
    rateLimitPerMinute: 60,
    maxRequestSize: 10,
    enableCaching: true,
    cacheExpiry: 300,
    backupFrequency: 'daily',
    retentionDays: 30,
    enableAutoCleanup: true,
  })

  const [activeTab, setActiveTab] = useState<'basic' | 'security' | 'api' | 'system'>('basic')
  const [isSaving, setIsSaving] = useState(false)

  // 更新设置
  const updateSetting = (key: keyof SystemSettings, value: any) => {
    setSettings(prev => ({ ...prev, [key]: value }))
  }

  // 保存设置
  const handleSave = async () => {
    setIsSaving(true)
    // 模拟保存过程
    await new Promise(resolve => setTimeout(resolve, 1000))
    setIsSaving(false)
  }

  // 重置设置
  const handleReset = () => {
    setSettings({
      siteName: 'AI Proxy Console',
      siteDescription: '智能代理服务管理控制台',
      language: 'zh',
      timezone: 'Asia/Shanghai',
      defaultTheme: 'light',
      primaryColor: '#7c3aed',
      allowUserTheme: true,
      sessionTimeout: 24,
      maxLoginAttempts: 5,
      enableTwoFactor: false,
      passwordMinLength: 8,
      emailNotifications: true,
      systemAlerts: true,
      maintenanceMode: false,
      rateLimitPerMinute: 60,
      maxRequestSize: 10,
      enableCaching: true,
      cacheExpiry: 300,
      backupFrequency: 'daily',
      retentionDays: 30,
      enableAutoCleanup: true,
    })
  }

  const tabs = [
    { id: 'basic', name: '基本设置', icon: Globe },
    { id: 'security', name: '安全设置', icon: Shield },
    { id: 'api', name: 'API设置', icon: Zap },
    { id: 'system', name: '系统设置', icon: Server },
  ]

  return (
    <div className="w-full">
      {/* 页面头部 */}
      <div className="flex items-center justify-between mb-6">
        <div>
          <h2 className="text-lg font-medium text-neutral-800">系统设置</h2>
          <p className="text-sm text-neutral-600 mt-1">管理系统配置和全局设置</p>
        </div>
        <div className="flex gap-2">
          <button
            onClick={handleReset}
            className="flex items-center gap-2 px-3 py-2 text-sm text-neutral-600 hover:text-neutral-800"
            title="重置设置"
          >
            <RefreshCw size={16} />
            重置
          </button>
          <button
            onClick={handleSave}
            disabled={isSaving}
            className="flex items-center gap-2 bg-violet-600 text-white px-4 py-2 rounded-lg text-sm font-medium hover:bg-violet-700 disabled:opacity-50"
          >
            <Save size={16} />
            {isSaving ? '保存中...' : '保存设置'}
          </button>
        </div>
      </div>

      {/* 系统信息统计 */}
      <div className="mb-6 grid grid-cols-1 md:grid-cols-3 gap-4">
        <div className="bg-white p-4 rounded-xl border border-neutral-200">
          <div className="text-sm text-violet-600">系统版本</div>
          <div className="text-2xl font-bold text-violet-900">v2.1.0</div>
        </div>
        <div className="bg-white p-4 rounded-xl border border-neutral-200">
          <div className="text-sm text-emerald-600">运行时间</div>
          <div className="text-2xl font-bold text-emerald-900">15天 6小时</div>
        </div>
        <div className="bg-white p-4 rounded-xl border border-neutral-200">
          <div className="text-sm text-blue-600">最后更新</div>
          <div className="text-2xl font-bold text-blue-900">2024-01-16</div>
        </div>
      </div>

      {/* 标签导航 */}
      <div className="bg-white rounded-2xl border border-neutral-200 overflow-hidden mb-6">
        <div className="border-b border-neutral-200">
          <div className="flex">
            {tabs.map((tab) => {
              const IconComponent = tab.icon
              return (
                <button
                  key={tab.id}
                  onClick={() => setActiveTab(tab.id as any)}
                  className={`flex items-center gap-2 px-6 py-4 text-sm font-medium border-b-2 transition-colors ${
                    activeTab === tab.id
                      ? 'border-violet-600 text-violet-600 bg-violet-50'
                      : 'border-transparent text-neutral-600 hover:text-neutral-800'
                  }`}
                >
                  <IconComponent size={16} />
                  {tab.name}
                </button>
              )
            })}
          </div>
        </div>

        <div className="p-6">
          {/* 基本设置 */}
          {activeTab === 'basic' && (
            <div className="space-y-6">
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div>
                  <label className="block text-sm font-medium text-neutral-700 mb-2">站点名称</label>
                  <input
                    type="text"
                    value={settings.siteName}
                    onChange={(e) => updateSetting('siteName', e.target.value)}
                    className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-neutral-700 mb-2">界面语言</label>
                  <ModernSelect
                    value={settings.language}
                    onValueChange={(value) => updateSetting('language', value)}
                    options={[
                      { value: 'zh', label: '中文' },
                      { value: 'en', label: 'English' }
                    ]}
                    placeholder="请选择语言"
                  />
                </div>
              </div>

              <div>
                <label className="block text-sm font-medium text-neutral-700 mb-2">站点描述</label>
                <textarea
                  value={settings.siteDescription}
                  onChange={(e) => updateSetting('siteDescription', e.target.value)}
                  rows={3}
                  className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
                />
              </div>

              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div>
                  <label className="block text-sm font-medium text-neutral-700 mb-2">默认主题</label>
                  <ModernSelect
                    value={settings.defaultTheme}
                    onValueChange={(value) => updateSetting('defaultTheme', value)}
                    options={[
                      { value: 'light', label: '浅色主题' },
                      { value: 'dark', label: '深色主题' },
                      { value: 'auto', label: '跟随系统' }
                    ]}
                    placeholder="请选择主题"
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-neutral-700 mb-2">时区</label>
                  <ModernSelect
                    value={settings.timezone}
                    onValueChange={(value) => updateSetting('timezone', value)}
                    options={[
                      { value: 'Asia/Shanghai', label: 'Asia/Shanghai (UTC+8)' },
                      { value: 'UTC', label: 'UTC (UTC+0)' },
                      { value: 'America/New_York', label: 'America/New_York (UTC-5)' }
                    ]}
                    placeholder="请选择时区"
                  />
                </div>
              </div>

              <div className="flex items-center justify-between p-4 bg-neutral-50 rounded-lg">
                <div>
                  <div className="text-sm font-medium text-neutral-900">允许用户自定义主题</div>
                  <div className="text-xs text-neutral-600">用户可以在个人设置中选择主题</div>
                </div>
                <label className="relative inline-flex items-center cursor-pointer">
                  <input
                    type="checkbox"
                    checked={settings.allowUserTheme}
                    onChange={(e) => updateSetting('allowUserTheme', e.target.checked)}
                    className="sr-only peer"
                  />
                  <div className="w-11 h-6 bg-neutral-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-violet-300 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-neutral-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-violet-600"></div>
                </label>
              </div>
            </div>
          )}

          {/* 安全设置 */}
          {activeTab === 'security' && (
            <div className="space-y-6">
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div>
                  <label className="block text-sm font-medium text-neutral-700 mb-2">会话超时（小时）</label>
                  <input
                    type="number"
                    min="1"
                    max="168"
                    value={settings.sessionTimeout}
                    onChange={(e) => updateSetting('sessionTimeout', parseInt(e.target.value))}
                    className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-neutral-700 mb-2">最大登录尝试次数</label>
                  <input
                    type="number"
                    min="3"
                    max="10"
                    value={settings.maxLoginAttempts}
                    onChange={(e) => updateSetting('maxLoginAttempts', parseInt(e.target.value))}
                    className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
                  />
                </div>
              </div>

              <div>
                <label className="block text-sm font-medium text-neutral-700 mb-2">密码最小长度</label>
                <input
                  type="number"
                  min="6"
                  max="32"
                  value={settings.passwordMinLength}
                  onChange={(e) => updateSetting('passwordMinLength', parseInt(e.target.value))}
                  className="w-full max-w-xs px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
                />
              </div>

              <div className="space-y-4">
                <div className="flex items-center justify-between p-4 bg-neutral-50 rounded-lg">
                  <div className="flex items-center gap-3">
                    <Lock size={16} className="text-neutral-400" />
                    <div>
                      <div className="text-sm font-medium text-neutral-900">启用双因素认证</div>
                      <div className="text-xs text-neutral-600">为管理员账户启用2FA</div>
                    </div>
                  </div>
                  <label className="relative inline-flex items-center cursor-pointer">
                    <input
                      type="checkbox"
                      checked={settings.enableTwoFactor}
                      onChange={(e) => updateSetting('enableTwoFactor', e.target.checked)}
                      className="sr-only peer"
                    />
                    <div className="w-11 h-6 bg-neutral-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-violet-300 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-neutral-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-violet-600"></div>
                  </label>
                </div>

                <div className="flex items-center justify-between p-4 bg-neutral-50 rounded-lg">
                  <div className="flex items-center gap-3">
                    <Bell size={16} className="text-neutral-400" />
                    <div>
                      <div className="text-sm font-medium text-neutral-900">系统安全警报</div>
                      <div className="text-xs text-neutral-600">异常登录和安全事件通知</div>
                    </div>
                  </div>
                  <label className="relative inline-flex items-center cursor-pointer">
                    <input
                      type="checkbox"
                      checked={settings.systemAlerts}
                      onChange={(e) => updateSetting('systemAlerts', e.target.checked)}
                      className="sr-only peer"
                    />
                    <div className="w-11 h-6 bg-neutral-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-violet-300 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-neutral-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-violet-600"></div>
                  </label>
                </div>
              </div>
            </div>
          )}

          {/* API设置 */}
          {activeTab === 'api' && (
            <div className="space-y-6">
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div>
                  <label className="block text-sm font-medium text-neutral-700 mb-2">速率限制（次/分钟）</label>
                  <input
                    type="number"
                    min="10"
                    max="1000"
                    value={settings.rateLimitPerMinute}
                    onChange={(e) => updateSetting('rateLimitPerMinute', parseInt(e.target.value))}
                    className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-neutral-700 mb-2">最大请求大小（MB）</label>
                  <input
                    type="number"
                    min="1"
                    max="100"
                    value={settings.maxRequestSize}
                    onChange={(e) => updateSetting('maxRequestSize', parseInt(e.target.value))}
                    className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
                  />
                </div>
              </div>

              <div>
                <label className="block text-sm font-medium text-neutral-700 mb-2">缓存过期时间（秒）</label>
                <input
                  type="number"
                  min="60"
                  max="3600"
                  value={settings.cacheExpiry}
                  onChange={(e) => updateSetting('cacheExpiry', parseInt(e.target.value))}
                  className="w-full max-w-xs px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
                />
              </div>

              <div className="flex items-center justify-between p-4 bg-neutral-50 rounded-lg">
                <div className="flex items-center gap-3">
                  <Zap size={16} className="text-neutral-400" />
                  <div>
                    <div className="text-sm font-medium text-neutral-900">启用响应缓存</div>
                    <div className="text-xs text-neutral-600">缓存API响应以提高性能</div>
                  </div>
                </div>
                <label className="relative inline-flex items-center cursor-pointer">
                  <input
                    type="checkbox"
                    checked={settings.enableCaching}
                    onChange={(e) => updateSetting('enableCaching', e.target.checked)}
                    className="sr-only peer"
                  />
                  <div className="w-11 h-6 bg-neutral-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-violet-300 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-neutral-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-violet-600"></div>
                </label>
              </div>
            </div>
          )}

          {/* 系统设置 */}
          {activeTab === 'system' && (
            <div className="space-y-6">
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div>
                  <label className="block text-sm font-medium text-neutral-700 mb-2">备份频率</label>
                  <ModernSelect
                    value={settings.backupFrequency}
                    onValueChange={(value) => updateSetting('backupFrequency', value)}
                    options={[
                      { value: 'daily', label: '每日' },
                      { value: 'weekly', label: '每周' },
                      { value: 'monthly', label: '每月' }
                    ]}
                    placeholder="请选择备份频率"
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-neutral-700 mb-2">数据保留天数</label>
                  <input
                    type="number"
                    min="7"
                    max="365"
                    value={settings.retentionDays}
                    onChange={(e) => updateSetting('retentionDays', parseInt(e.target.value))}
                    className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
                  />
                </div>
              </div>

              <div className="space-y-4">
                <div className="flex items-center justify-between p-4 bg-neutral-50 rounded-lg">
                  <div className="flex items-center gap-3">
                    <Database size={16} className="text-neutral-400" />
                    <div>
                      <div className="text-sm font-medium text-neutral-900">自动数据清理</div>
                      <div className="text-xs text-neutral-600">自动删除过期的请求记录和临时文件</div>
                    </div>
                  </div>
                  <label className="relative inline-flex items-center cursor-pointer">
                    <input
                      type="checkbox"
                      checked={settings.enableAutoCleanup}
                      onChange={(e) => updateSetting('enableAutoCleanup', e.target.checked)}
                      className="sr-only peer"
                    />
                    <div className="w-11 h-6 bg-neutral-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-violet-300 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-neutral-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-violet-600"></div>
                  </label>
                </div>

                <div className="flex items-center justify-between p-4 bg-neutral-50 rounded-lg">
                  <div className="flex items-center gap-3">
                    <Mail size={16} className="text-neutral-400" />
                    <div>
                      <div className="text-sm font-medium text-neutral-900">邮件通知</div>
                      <div className="text-xs text-neutral-600">发送系统通知和报告邮件</div>
                    </div>
                  </div>
                  <label className="relative inline-flex items-center cursor-pointer">
                    <input
                      type="checkbox"
                      checked={settings.emailNotifications}
                      onChange={(e) => updateSetting('emailNotifications', e.target.checked)}
                      className="sr-only peer"
                    />
                    <div className="w-11 h-6 bg-neutral-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-violet-300 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-neutral-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-violet-600"></div>
                  </label>
                </div>

                <div className="flex items-center justify-between p-4 bg-yellow-50 rounded-lg border border-yellow-200">
                  <div className="flex items-center gap-3">
                    <Settings size={16} className="text-yellow-600" />
                    <div>
                      <div className="text-sm font-medium text-yellow-900">维护模式</div>
                      <div className="text-xs text-yellow-700">启用后，系统将显示维护页面</div>
                    </div>
                  </div>
                  <label className="relative inline-flex items-center cursor-pointer">
                    <input
                      type="checkbox"
                      checked={settings.maintenanceMode}
                      onChange={(e) => updateSetting('maintenanceMode', e.target.checked)}
                      className="sr-only peer"
                    />
                    <div className="w-11 h-6 bg-yellow-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-yellow-300 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-yellow-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-yellow-600"></div>
                  </label>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>

    </div>
  )
}

export default SettingsPage
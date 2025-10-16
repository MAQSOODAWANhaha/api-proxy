/**
 * Settings.tsx
 * 系统设置页：聚焦当前产品可落地的核心配置项
 */

import React, { useState } from 'react'
import SystemInfo from '@/components/SystemInfo'
import ModernSelect from '../components/common/ModernSelect'
import { RefreshCw, Save, Server, Settings, Shield, Zap } from 'lucide-react'

interface SystemSettings {
  // 基础信息
  siteName: string
  language: 'zh' | 'en'
  timezone: string
  defaultTheme: 'light' | 'dark' | 'auto'
  allowUserTheme: boolean

  // 安全策略
  sessionTimeout: number
  maxLoginAttempts: number
  enableTwoFactor: boolean

  // API 配额
  rateLimitPerMinute: number
  maxRequestSize: number

  // 系统状态
  maintenanceMode: boolean
}

const INITIAL_SETTINGS: SystemSettings = {
  siteName: 'AI Proxy Console',
  language: 'zh',
  timezone: 'Asia/Shanghai',
  defaultTheme: 'light',
  allowUserTheme: true,
  sessionTimeout: 24,
  maxLoginAttempts: 5,
  enableTwoFactor: false,
  rateLimitPerMinute: 60,
  maxRequestSize: 10,
  maintenanceMode: false,
}

const SettingsPage: React.FC = () => {
  const [settings, setSettings] = useState<SystemSettings>(INITIAL_SETTINGS)
  const [activeTab, setActiveTab] = useState<'basic' | 'security' | 'api' | 'system'>('basic')
  const [isSaving, setIsSaving] = useState(false)

  const updateSetting = <K extends keyof SystemSettings>(key: K, value: SystemSettings[K]) => {
    setSettings(prev => ({ ...prev, [key]: value }))
  }

  const handleSave = async () => {
    setIsSaving(true)
    await new Promise(resolve => setTimeout(resolve, 800))
    setIsSaving(false)
  }

  const handleReset = () => setSettings(INITIAL_SETTINGS)

  const tabs = [
    { id: 'basic', name: '基础信息', icon: Settings },
    { id: 'security', name: '安全策略', icon: Shield },
    { id: 'api', name: 'API 配额', icon: Zap },
    { id: 'system', name: '系统状态', icon: Server },
  ] as const

  return (
    <div className="w-full">
      <div className="flex items-center justify-between mb-6">
        <div>
          <h2 className="text-lg font-medium text-neutral-800">系统设置</h2>
          <p className="text-sm text-neutral-600 mt-1">管理控制台的全局参数与运行策略</p>
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

      <div className="mb-6">
        <SystemInfo />
      </div>

      <div className="bg-white rounded-2xl border border-neutral-200 overflow-hidden">
        <div className="border-b border-neutral-200">
          <div className="flex">
            {tabs.map(tab => {
              const Icon = tab.icon
              const isActive = activeTab === tab.id
              return (
                <button
                  key={tab.id}
                  onClick={() => setActiveTab(tab.id)}
                  className={`flex items-center gap-2 px-6 py-4 text-sm font-medium border-b-2 transition-colors ${
                    isActive
                      ? 'border-violet-600 text-violet-600 bg-violet-50'
                      : 'border-transparent text-neutral-600 hover:text-neutral-800'
                  }`}
                >
                  <Icon size={16} />
                  {tab.name}
                </button>
              )
            })}
          </div>
        </div>

        <div className="p-6 space-y-6">
          {activeTab === 'basic' && (
            <div className="space-y-6">
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div>
                  <label className="block text-sm font-medium text-neutral-700 mb-2">站点名称</label>
                  <input
                    type="text"
                    value={settings.siteName}
                    onChange={e => updateSetting('siteName', e.target.value)}
                    className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-neutral-700 mb-2">界面语言</label>
                  <ModernSelect
                    value={settings.language}
                    onValueChange={value => updateSetting('language', value as SystemSettings['language'])}
                    options={[
                      { value: 'zh', label: '中文' },
                      { value: 'en', label: 'English' },
                    ]}
                    placeholder="请选择语言"
                  />
                </div>
              </div>

              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div>
                  <label className="block text-sm font-medium text-neutral-700 mb-2">默认主题</label>
                  <ModernSelect
                    value={settings.defaultTheme}
                    onValueChange={value =>
                      updateSetting('defaultTheme', value as SystemSettings['defaultTheme'])
                    }
                    options={[
                      { value: 'light', label: '浅色' },
                      { value: 'dark', label: '深色' },
                      { value: 'auto', label: '跟随系统' },
                    ]}
                    placeholder="请选择主题"
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-neutral-700 mb-2">时区</label>
                  <ModernSelect
                    value={settings.timezone}
                    onValueChange={value => updateSetting('timezone', value)}
                    options={[
                      { value: 'Asia/Shanghai', label: 'Asia/Shanghai (UTC+8)' },
                      { value: 'UTC', label: 'UTC (UTC+0)' },
                      { value: 'America/New_York', label: 'America/New_York (UTC-5)' },
                    ]}
                    placeholder="请选择时区"
                  />
                </div>
              </div>

              <div className="flex items-center justify-between p-4 bg-neutral-50 rounded-lg">
                <div>
                  <div className="text-sm font-medium text-neutral-900">允许用户自定义主题</div>
                  <div className="text-xs text-neutral-600">用户可以在个人设置中选择浅色或深色模式</div>
                </div>
                <label className="relative inline-flex items-center cursor-pointer">
                  <input
                    type="checkbox"
                    checked={settings.allowUserTheme}
                    onChange={e => updateSetting('allowUserTheme', e.target.checked)}
                    className="sr-only peer"
                  />
                  <div className="w-11 h-6 bg-neutral-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-violet-300 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-neutral-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-violet-600"></div>
                </label>
              </div>
            </div>
          )}

          {activeTab === 'security' && (
            <div className="space-y-6">
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div>
                  <label className="block text-sm font-medium text-neutral-700 mb-2">会话超时（小时）</label>
                  <input
                    type="number"
                    min={1}
                    max={168}
                    value={settings.sessionTimeout}
                    onChange={e => updateSetting('sessionTimeout', Number(e.target.value) || 1)}
                    className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
                  />
                  <p className="mt-1 text-xs text-neutral-500">超时后需要重新登录，建议 24 小时以内</p>
                </div>
                <div>
                  <label className="block text-sm font-medium text-neutral-700 mb-2">最大登录尝试次数</label>
                  <input
                    type="number"
                    min={3}
                    max={10}
                    value={settings.maxLoginAttempts}
                    onChange={e => updateSetting('maxLoginAttempts', Number(e.target.value) || 3)}
                    className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
                  />
                </div>
              </div>

              <div className="flex items-center justify-between p-4 bg-neutral-50 rounded-lg">
                <div>
                  <div className="text-sm font-medium text-neutral-900">启用双因素认证</div>
                  <div className="text-xs text-neutral-600">对管理员账户启用二次校验，提升安全性</div>
                </div>
                <label className="relative inline-flex items-center cursor-pointer">
                  <input
                    type="checkbox"
                    checked={settings.enableTwoFactor}
                    onChange={e => updateSetting('enableTwoFactor', e.target.checked)}
                    className="sr-only peer"
                  />
                  <div className="w-11 h-6 bg-neutral-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-violet-300 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-neutral-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-violet-600"></div>
                </label>
              </div>
            </div>
          )}

          {activeTab === 'api' && (
            <div className="space-y-6">
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div>
                  <label className="block text-sm font-medium text-neutral-700 mb-2">速率限制（次/分钟）</label>
                  <input
                    type="number"
                    min={10}
                    max={1000}
                    value={settings.rateLimitPerMinute}
                    onChange={e => updateSetting('rateLimitPerMinute', Number(e.target.value) || 10)}
                    className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-neutral-700 mb-2">最大请求大小（MB）</label>
                  <input
                    type="number"
                    min={1}
                    max={100}
                    value={settings.maxRequestSize}
                    onChange={e => updateSetting('maxRequestSize', Number(e.target.value) || 1)}
                    className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
                  />
                </div>
              </div>
              <p className="text-xs text-neutral-500">
                以上参数用于指导后端限流与上传策略，后续在系统设置生效前请同步运营团队。
              </p>
            </div>
          )}

          {activeTab === 'system' && (
            <div className="space-y-4">
              <div className="flex items-center justify-between p-4 bg-yellow-50 rounded-lg border border-yellow-200">
                <div>
                  <div className="text-sm font-medium text-yellow-900">维护模式</div>
                  <div className="text-xs text-yellow-700">
                    启用后仅管理员可访问控制台，用于迁移或紧急修复
                  </div>
                </div>
                <label className="relative inline-flex items-center cursor-pointer">
                  <input
                    type="checkbox"
                    checked={settings.maintenanceMode}
                    onChange={e => updateSetting('maintenanceMode', e.target.checked)}
                    className="sr-only peer"
                  />
                  <div className="w-11 h-6 bg-yellow-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-yellow-300 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-yellow-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-yellow-600"></div>
                </label>
              </div>
              <p className="text-xs text-neutral-500">
                维护模式仅修改系统状态标记，真正的流量切换仍需配合部署和告警接入。
              </p>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

export default SettingsPage

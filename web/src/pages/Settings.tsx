/**
 * Settings.tsx
 * 系统设置页：聚焦当前产品可落地的核心配置项
 */

import React, { useState } from 'react'
import SystemInfo from '@/components/SystemInfo'
import { RefreshCw, Save, Shield, Key, Plus, X } from 'lucide-react'

interface SystemSettings {
  // 安全策略
  sessionTimeout: number
  maxLoginAttempts: number
  enableTwoFactor: boolean

  // 访问控制
  allowedIps: string[]
  deniedIps: string[]
}

const INITIAL_SETTINGS: SystemSettings = {
  sessionTimeout: 24,
  maxLoginAttempts: 5,
  enableTwoFactor: false,
  allowedIps: ['127.0.0.1/32', '::1/128'],
  deniedIps: [],
}

const SettingsPage: React.FC = () => {
  const [settings, setSettings] = useState<SystemSettings>(INITIAL_SETTINGS)
  const [activeTab, setActiveTab] = useState<'security' | 'access'>('security')
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
    { id: 'security', name: '安全策略', icon: Shield },
    { id: 'access', name: '访问控制', icon: Key },
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

          {activeTab === 'access' && (
            <div className="space-y-6">
              {/* 访问控制说明 */}
              <div className="bg-blue-50 p-4 rounded-lg border border-blue-200">
                <div className="flex items-start gap-3">
                  <Key size={18} className="text-blue-600 mt-0.5" />
                  <div>
                    <div className="text-sm font-medium text-blue-900 mb-1">访问控制策略</div>
                    <div className="text-xs text-blue-700">
                      管理端口的IP访问控制列表。允许列表优先于拒绝列表，支持CIDR格式。
                      只有符合允许列表且不在拒绝列表中的IP地址才能访问管理界面。
                    </div>
                  </div>
                </div>
              </div>

              {/* 允许列表 */}
              <div>
                <div className="flex items-center justify-between mb-3">
                  <div className="flex items-center gap-2">
                    <div className="w-2 h-2 bg-green-500 rounded-full"></div>
                    <label className="text-sm font-medium text-neutral-900">允许IP列表</label>
                    <span className="text-xs text-neutral-500">（允许访问管理界面的IP地址）</span>
                  </div>
                </div>
                <div className="space-y-2">
                  {settings.allowedIps.map((ip, index) => (
                    <div key={`allowed-${index}`} className="flex items-center gap-2">
                      <div className="flex-1 flex items-center gap-2 px-3 py-2 bg-green-50 border border-green-200 rounded-lg">
                        <span className="text-sm text-green-800">{ip}</span>
                        <span className="text-xs text-green-600 bg-green-100 px-2 py-0.5 rounded">允许</span>
                      </div>
                      <button
                        onClick={() => {
                          const newAllowedIps = settings.allowedIps.filter((_, i) => i !== index)
                          updateSetting('allowedIps', newAllowedIps)
                        }}
                        className="p-1 text-red-600 hover:text-red-800 hover:bg-red-50 rounded"
                        title="删除"
                      >
                        <X size={16} />
                      </button>
                    </div>
                  ))}
                  <div className="flex items-center gap-2">
                    <input
                      type="text"
                      placeholder="输入IP地址或CIDR（如 192.168.1.0/24）"
                      className="flex-1 px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
                      id="allowed-ip-input"
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') {
                          const value = e.currentTarget.value.trim()
                          if (value && !settings.allowedIps.includes(value)) {
                            updateSetting('allowedIps', [...settings.allowedIps, value])
                            e.currentTarget.value = ''
                          }
                        }
                      }}
                    />
                    <button
                      onClick={() => {
                        const input = document.getElementById('allowed-ip-input') as HTMLInputElement
                        const value = input?.value?.trim()
                        if (value && !settings.allowedIps.includes(value)) {
                          updateSetting('allowedIps', [...settings.allowedIps, value])
                          if (input) input.value = ''
                        }
                      }}
                      className="flex items-center gap-1 px-3 py-2 bg-green-600 text-white rounded-lg text-sm hover:bg-green-700"
                    >
                      <Plus size={16} />
                      添加
                    </button>
                  </div>
                </div>
              </div>

              {/* 拒绝列表 */}
              <div>
                <div className="flex items-center justify-between mb-3">
                  <div className="flex items-center gap-2">
                    <div className="w-2 h-2 bg-red-500 rounded-full"></div>
                    <label className="text-sm font-medium text-neutral-900">拒绝IP列表</label>
                    <span className="text-xs text-neutral-500">（禁止访问管理界面的IP地址）</span>
                  </div>
                </div>
                <div className="space-y-2">
                  {settings.deniedIps.map((ip, index) => (
                    <div key={`denied-${index}`} className="flex items-center gap-2">
                      <div className="flex-1 flex items-center gap-2 px-3 py-2 bg-red-50 border border-red-200 rounded-lg">
                        <span className="text-sm text-red-800">{ip}</span>
                        <span className="text-xs text-red-600 bg-red-100 px-2 py-0.5 rounded">拒绝</span>
                      </div>
                      <button
                        onClick={() => {
                          const newDeniedIps = settings.deniedIps.filter((_, i) => i !== index)
                          updateSetting('deniedIps', newDeniedIps)
                        }}
                        className="p-1 text-red-600 hover:text-red-800 hover:bg-red-50 rounded"
                        title="删除"
                      >
                        <X size={16} />
                      </button>
                    </div>
                  ))}
                  <div className="flex items-center gap-2">
                    <input
                      type="text"
                      placeholder="输入IP地址或CIDR（如 10.0.0.0/8）"
                      className="flex-1 px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
                      id="denied-ip-input"
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') {
                          const value = e.currentTarget.value.trim()
                          if (value && !settings.deniedIps.includes(value)) {
                            updateSetting('deniedIps', [...settings.deniedIps, value])
                            e.currentTarget.value = ''
                          }
                        }
                      }}
                    />
                    <button
                      onClick={() => {
                        const input = document.getElementById('denied-ip-input') as HTMLInputElement
                        const value = input?.value?.trim()
                        if (value && !settings.deniedIps.includes(value)) {
                          updateSetting('deniedIps', [...settings.deniedIps, value])
                          if (input) input.value = ''
                        }
                      }}
                      className="flex items-center gap-1 px-3 py-2 bg-red-600 text-white rounded-lg text-sm hover:bg-red-700"
                    >
                      <Plus size={16} />
                      添加
                    </button>
                  </div>
                </div>
              </div>

              {/* 常用CIDR示例 */}
              <div className="bg-neutral-50 p-4 rounded-lg">
                <div className="text-sm font-medium text-neutral-900 mb-3">常用CIDR示例</div>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-2 text-xs">
                  <div className="flex items-center justify-between p-2 bg-white rounded border">
                    <span className="text-neutral-700">192.168.1.0/24</span>
                    <span className="text-neutral-500">私有网络 /24</span>
                  </div>
                  <div className="flex items-center justify-between p-2 bg-white rounded border">
                    <span className="text-neutral-700">10.0.0.0/8</span>
                    <span className="text-neutral-500">私有网络 /8</span>
                  </div>
                  <div className="flex items-center justify-between p-2 bg-white rounded border">
                    <span className="text-neutral-700">172.16.0.0/12</span>
                    <span className="text-neutral-500">私有网络 /12</span>
                  </div>
                  <div className="flex items-center justify-between p-2 bg-white rounded border">
                    <span className="text-neutral-700">127.0.0.1/32</span>
                    <span className="text-neutral-500">本机回环</span>
                  </div>
                </div>
              </div>

              <p className="text-xs text-neutral-500">
                访问控制列表修改后需要重启管理服务才能生效。请确保您没有被锁定在系统之外。
              </p>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

export default SettingsPage

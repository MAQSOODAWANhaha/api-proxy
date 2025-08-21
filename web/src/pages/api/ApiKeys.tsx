/**
 * ApiUserKeys.tsx
 * 用户 API Keys 管理页：完整的增删改查和统计功能
 */

import React, { useState, useMemo, useEffect } from 'react'
import {
  Search,
  Plus,
  Edit,
  Trash2,
  BarChart3,
  Eye,
  EyeOff,
  Filter,
  RefreshCw,
  Copy,
  ChevronLeft,
  ChevronRight,
  ChevronDown,
  X,
  Check,
  Key,
  Activity,
  Users,
} from 'lucide-react'
import { StatCard } from '../../components/common/StatCard'
import FilterSelect from '../../components/common/FilterSelect'
import ModernSelect from '../../components/common/ModernSelect'
import MultiSelect from '../../components/common/MultiSelect'
import api, { 
  UserServiceApiKey, 
  UserServiceCardsResponse,
  UserServiceApiKeysResponse,
  CreateUserServiceApiKeyRequest,
  UpdateUserServiceApiKeyRequest,
  UserServiceApiKeyUsageResponse,
  UserServiceApiKeyDetail
} from '../../lib/api'

/** API Key 数据结构 - 适配后端API */
interface ApiKey {
  id: number
  keyName: string
  keyValue: string
  description: string
  providerType: string
  schedulingStrategy: 'round_robin' | 'priority' | 'weighted' | 'random'
  providerKeys: number[]
  retryCount: number
  timeoutSeconds: number
  rateLimitPerMinute: number
  tokenLimitPerDay: number
  status: 'active' | 'disabled'
  usage: number
  limit: number
  createdAt: string
  lastUsed: string
}

/** 将后端API数据转换为前端数据结构 */
const transformApiKeyFromBackend = (backendKey: UserServiceApiKey): ApiKey => ({
  id: backendKey.id,
  keyName: backendKey.name,
  keyValue: backendKey.api_key,
  description: backendKey.description,
  providerType: backendKey.provider,
  schedulingStrategy: 'round_robin', // 默认值，需要从详情API获取
  providerKeys: [], // 需要从详情API获取
  retryCount: 3, // 默认值，需要从详情API获取
  timeoutSeconds: 30, // 默认值，需要从详情API获取
  rateLimitPerMinute: 60, // 默认值，需要从详情API获取
  tokenLimitPerDay: 10000, // 默认值，需要从详情API获取
  status: backendKey.is_active ? 'active' : 'disabled',
  usage: backendKey.usage?.success || 0,
  limit: backendKey.usage ? backendKey.usage.success + backendKey.usage.failure : 0,
  createdAt: new Date(backendKey.created_at).toLocaleDateString(),
  lastUsed: backendKey.last_used_at ? new Date(backendKey.last_used_at).toLocaleString() : '从未使用',
})

/** 弹窗类型 */
type DialogType = 'add' | 'edit' | 'delete' | 'stats' | 'usage' | 'detail' | 'regenerate' | null

/** 页面主组件 */
const ApiUserKeysPage: React.FC = () => {
  // API数据状态
  const [data, setData] = useState<ApiKey[]>([])
  const [cardsData, setCardsData] = useState<UserServiceCardsResponse | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  
  // UI状态
  const [searchTerm, setSearchTerm] = useState('')
  const [statusFilter, setStatusFilter] = useState<'all' | 'active' | 'disabled'>('all')
  const [selectedItem, setSelectedItem] = useState<ApiKey | null>(null)
  const [dialogType, setDialogType] = useState<DialogType>(null)
  const [showKeyValues, setShowKeyValues] = useState<{ [key: number]: boolean }>({})
  
  // 分页状态
  const [currentPage, setCurrentPage] = useState(1)
  const [pageSize, setPageSize] = useState(10)
  const [totalItems, setTotalItems] = useState(0)

  // API调用函数
  const loadCardsData = async () => {
    try {
      const response = await api.userService.getCards()
      if (response.success && response.data) {
        setCardsData(response.data)
      } else {
        console.error('Failed to load cards data:', response.error?.message)
      }
    } catch (error) {
      console.error('Error loading cards data:', error)
    }
  }

  const loadKeysData = async () => {
    try {
      setLoading(true)
      const isActiveFilter = statusFilter === 'all' ? undefined : statusFilter === 'active'
      const response = await api.userService.getKeys({
        page: currentPage,
        limit: pageSize,
        name: searchTerm || undefined,
        is_active: isActiveFilter
      })
      
      if (response.success && response.data) {
        const transformedKeys = response.data.service_api_keys.map(transformApiKeyFromBackend)
        setData(transformedKeys)
        setTotalItems(response.data.pagination.total)
        setError(null)
      } else {
        setError(response.error?.message || '获取API Keys失败')
        console.error('Failed to load keys data:', response.error?.message)
      }
    } catch (error) {
      setError('网络错误，请稍后重试')
      console.error('Error loading keys data:', error)
    } finally {
      setLoading(false)
    }
  }

  const refreshData = async () => {
    await Promise.all([loadCardsData(), loadKeysData()])
  }

  // 初始数据加载
  useEffect(() => {
    refreshData()
  }, [currentPage, pageSize, searchTerm, statusFilter])

  // 服务端已处理筛选和分页，直接使用数据
  const filteredData = data
  const paginatedData = data
  const totalPages = Math.ceil(totalItems / pageSize)
  
  // 重置页码当过滤条件改变时
  React.useEffect(() => {
    setCurrentPage(1)
  }, [searchTerm, statusFilter])

  // 添加新API Key
  const handleAdd = async (newKey: Omit<ApiKey, 'id' | 'usage' | 'createdAt' | 'lastUsed' | 'keyValue'>) => {
    try {
      const createRequest: CreateUserServiceApiKeyRequest = {
        name: newKey.keyName,
        description: newKey.description,
        provider_type_id: 1, // 需要从实际的服务商映射
        user_provider_keys_ids: newKey.providerKeys,
        scheduling_strategy: newKey.schedulingStrategy,
        retry_count: newKey.retryCount,
        timeout_seconds: newKey.timeoutSeconds,
        max_request_per_min: newKey.rateLimitPerMinute,
        max_tokens_per_day: newKey.tokenLimitPerDay,
        is_active: newKey.status === 'active'
      }
      
      const response = await api.userService.createKey(createRequest)
      if (response.success) {
        setDialogType(null)
        await refreshData() // 重新加载数据
      } else {
        console.error('Failed to create API key:', response.error?.message)
        setError(response.error?.message || '创建API Key失败')
      }
    } catch (error) {
      console.error('Error creating API key:', error)
      setError('创建API Key失败，请稍后重试')
    }
  }

  // 编辑API Key
  const handleEdit = async (updatedKey: ApiKey) => {
    try {
      const updateRequest: UpdateUserServiceApiKeyRequest = {
        name: updatedKey.keyName,
        description: updatedKey.description,
        user_provider_keys_ids: updatedKey.providerKeys,
        scheduling_strategy: updatedKey.schedulingStrategy,
        retry_count: updatedKey.retryCount,
        timeout_seconds: updatedKey.timeoutSeconds,
        max_request_per_min: updatedKey.rateLimitPerMinute,
        max_tokens_per_day: updatedKey.tokenLimitPerDay,
      }
      
      const response = await api.userService.updateKey(updatedKey.id, updateRequest)
      if (response.success) {
        setDialogType(null)
        setSelectedItem(null)
        await refreshData() // 重新加载数据
      } else {
        console.error('Failed to update API key:', response.error?.message)
        setError(response.error?.message || '更新API Key失败')
      }
    } catch (error) {
      console.error('Error updating API key:', error)
      setError('更新API Key失败，请稍后重试')
    }
  }

  // 删除API Key
  const handleDelete = async () => {
    if (!selectedItem) return
    
    try {
      const response = await api.userService.deleteKey(selectedItem.id)
      if (response.success) {
        setDialogType(null)
        setSelectedItem(null)
        await refreshData() // 重新加载数据
      } else {
        console.error('Failed to delete API key:', response.error?.message)
        setError(response.error?.message || '删除API Key失败')
      }
    } catch (error) {
      console.error('Error deleting API key:', error)
      setError('删除API Key失败，请稍后重试')
    }
  }

  // 切换API Key可见性
  const toggleKeyVisibility = (id: number) => {
    setShowKeyValues(prev => ({ ...prev, [id]: !prev[id] }))
  }

  // 复制到剪贴板
  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text)
  }

  // 重新生成API Key
  const handleRegenerate = async (id: number) => {
    try {
      setLoading(true)
      const response = await api.userService.regenerateKey(id)
      if (response.success) {
        await refreshData() // 重新加载数据
        setDialogType(null)
        setSelectedItem(null)
        setError(null)
      } else {
        console.error('Failed to regenerate API key:', response.error?.message)
        setError(response.error?.message || '重新生成API Key失败')
      }
    } catch (error) {
      console.error('Error regenerating API key:', error)
      setError('重新生成API Key失败，请稍后重试')
    } finally {
      setLoading(false)
    }
  }

  // 切换API Key状态（启用/禁用）
  const handleToggleStatus = async (id: number, currentStatus: boolean) => {
    try {
      setLoading(true)
      const response = await api.userService.updateKeyStatus(id, { is_active: !currentStatus })
      if (response.success) {
        await refreshData() // 重新加载数据
        setError(null)
      } else {
        console.error('Failed to toggle API key status:', response.error?.message)
        setError(response.error?.message || '切换API Key状态失败')
      }
    } catch (error) {
      console.error('Error toggling API key status:', error)
      setError('切换API Key状态失败，请稍后重试')
    } finally {
      setLoading(false)
    }
  }

  // 查看API Key使用统计
  const [usageData, setUsageData] = useState<any>(null)
  const [usageLoading, setUsageLoading] = useState(false)

  const handleViewUsage = async (id: number) => {
    try {
      setUsageLoading(true)
      const response = await api.userService.getKeyUsage(id, {
        time_range: '30days'
      })
      if (response.success && response.data) {
        setUsageData(response.data)
        setDialogType('usage')
        setError(null)
      } else {
        console.error('Failed to load usage data:', response.error?.message)
        setError(response.error?.message || '获取使用统计失败')
      }
    } catch (error) {
      console.error('Error loading usage data:', error)
      setError('获取使用统计失败，请稍后重试')
    } finally {
      setUsageLoading(false)
    }
  }

  // 查看API Key详情
  const [keyDetail, setKeyDetail] = useState<UserServiceApiKeyDetail | null>(null)
  const [detailLoading, setDetailLoading] = useState(false)

  const handleViewDetail = async (id: number) => {
    try {
      setDetailLoading(true)
      const response = await api.userService.getKey(id)
      if (response.success && response.data) {
        setKeyDetail(response.data)
        setDialogType('detail')
        setError(null)
      } else {
        console.error('Failed to load key detail:', response.error?.message)
        setError(response.error?.message || '获取API Key详情失败')
      }
    } catch (error) {
      console.error('Error loading key detail:', error)
      setError('获取API Key详情失败，请稍后重试')
    } finally {
      setDetailLoading(false)
    }
  }

  // 渲染遮罩的API Key
  const renderMaskedKey = (key: string, id: number) => {
    const isVisible = showKeyValues[id]
    return (
      <div className="flex items-center gap-2">
        <code className="font-mono text-xs bg-neutral-100 px-2 py-1 rounded">
          {isVisible ? key : `${key.substring(0, 8)}...${key.substring(key.length - 4)}`}
        </code>
        <button
          onClick={() => toggleKeyVisibility(id)}
          className="text-neutral-500 hover:text-neutral-700"
          title={isVisible ? '隐藏' : '显示'}
        >
          {isVisible ? <EyeOff size={14} /> : <Eye size={14} />}
        </button>
        <button
          onClick={() => copyToClipboard(key)}
          className="text-neutral-500 hover:text-neutral-700"
          title="复制"
        >
          <Copy size={14} />
        </button>
      </div>
    )
  }

  return (
    <div className="w-full">
      {/* 页面头部 */}
      <div className="flex items-center justify-between mb-6">
        <div>
          <h2 className="text-lg font-medium text-neutral-800">用户 API Keys</h2>
          <p className="text-sm text-neutral-600 mt-1">管理用户的API访问密钥</p>
        </div>
        <div className="flex gap-2">
          <button
            onClick={refreshData}
            className="flex items-center gap-2 px-3 py-2 text-sm text-neutral-600 hover:text-neutral-800 disabled:opacity-50"
            title="刷新数据"
            disabled={loading}
          >
            <RefreshCw size={16} className={loading ? 'animate-spin' : ''} />
            刷新
          </button>
          <button
            onClick={() => setDialogType('add')}
            className="flex items-center gap-2 bg-violet-600 text-white px-4 py-2 rounded-lg text-sm font-medium hover:bg-violet-700"
          >
            <Plus size={16} />
            新增密钥
          </button>
        </div>
      </div>

      {/* 错误提示 */}
      {error && (
        <div className="mb-4 p-4 bg-red-50 border border-red-200 rounded-lg text-red-700">
          <p className="text-sm">{error}</p>
          <button 
            onClick={() => setError(null)}
            className="mt-2 text-xs underline"
          >
            关闭
          </button>
        </div>
      )}

      {/* 统计信息 */}
      <div className="mb-6 grid grid-cols-1 md:grid-cols-3 gap-4">
        <StatCard
          icon={<Key size={16} />}
          value={cardsData?.total_api_keys?.toString() || '0'}
          label="总密钥数"
          color="#7c3aed"
        />
        <StatCard
          icon={<Activity size={16} />}
          value={cardsData?.active_api_keys?.toString() || '0'}
          label="活跃密钥"
          color="#10b981"
        />
        <StatCard
          icon={<Users size={16} />}
          value={cardsData?.requests?.toLocaleString() || '0'}
          label="总使用次数"
          color="#0ea5e9"
        />
      </div>

      {/* 搜索和过滤 */}
      <div className="flex items-center gap-4 mb-4">
        <div className="relative flex-1 max-w-md">
          <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 text-neutral-400" size={16} />
          <input
            type="text"
            placeholder="搜索密钥名称、描述或服务商..."
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            className="w-full pl-10 pr-4 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>
        <div className="flex items-center gap-2">
          <FilterSelect
            value={statusFilter}
            onValueChange={(value) => setStatusFilter(value as 'all' | 'active' | 'disabled')}
            options={[
              { value: 'all', label: '全部状态' },
              { value: 'active', label: '启用' },
              { value: 'disabled', label: '停用' }
            ]}
            placeholder="全部状态"
          />
        </div>
      </div>

      {/* 数据表格 */}
      <div className="bg-white rounded-2xl border border-neutral-200 overflow-hidden hover:shadow-sm transition-shadow">
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead className="bg-neutral-50 text-neutral-600">
              <tr>
                <th className="px-4 py-3 text-left font-medium">密钥名称</th>
                <th className="px-4 py-3 text-left font-medium">描述</th>
                <th className="px-4 py-3 text-left font-medium">服务商</th>
                <th className="px-4 py-3 text-left font-medium">API Key</th>
                <th className="px-4 py-3 text-left font-medium">使用情况</th>
                <th className="px-4 py-3 text-left font-medium">状态</th>
                <th className="px-4 py-3 text-left font-medium">最后使用</th>
                <th className="px-4 py-3 text-left font-medium">操作</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-neutral-200">
              {loading ? (
                <tr>
                  <td colSpan={8} className="px-4 py-8 text-center">
                    <div className="flex items-center justify-center gap-2 text-neutral-500">
                      <RefreshCw size={16} className="animate-spin" />
                      <span>加载中...</span>
                    </div>
                  </td>
                </tr>
              ) : paginatedData.length === 0 ? (
                <tr>
                  <td colSpan={8} className="px-4 py-8 text-center text-neutral-500">
                    暂无数据
                  </td>
                </tr>
              ) : (
                paginatedData.map((item) => (
                <tr key={item.id} className="text-neutral-800 hover:bg-neutral-50">
                  <td className="px-4 py-3">
                    <div>
                      <div className="font-medium">{item.keyName}</div>
                      <div className="text-xs text-neutral-500">创建于 {item.createdAt}</div>
                    </div>
                  </td>
                  <td className="px-4 py-3">
                    <div className="max-w-xs truncate" title={item.description}>
                      {item.description || '无描述'}
                    </div>
                  </td>
                  <td className="px-4 py-3">
                    <span className="px-2 py-1 bg-neutral-100 text-neutral-700 rounded text-xs font-medium">
                      {item.providerType}
                    </span>
                  </td>
                  <td className="px-4 py-3">{renderMaskedKey(item.keyValue, item.id)}</td>
                  <td className="px-4 py-3">
                    <div className="flex items-center gap-2">
                      <span className="text-sm">{item.usage.toLocaleString()} / {item.limit.toLocaleString()}</span>
                      <button
                        onClick={() => {
                          setSelectedItem(item)
                          setDialogType('stats')
                        }}
                        className="text-violet-600 hover:text-violet-700"
                        title="查看统计"
                      >
                        <BarChart3 size={14} />
                      </button>
                    </div>
                    <div className="w-full bg-neutral-200 rounded-full h-1.5 mt-1">
                      <div
                        className="bg-violet-600 h-1.5 rounded-full"
                        style={{ width: `${Math.min((item.usage / item.limit) * 100, 100)}%` }}
                      />
                    </div>
                  </td>
                  <td className="px-4 py-3">
                    <span
                      className={`inline-flex items-center px-2 py-1 rounded-full text-xs font-medium ${
                        item.status === 'active'
                          ? 'bg-emerald-50 text-emerald-700 ring-1 ring-emerald-200'
                          : 'bg-neutral-100 text-neutral-700 ring-1 ring-neutral-300'
                      }`}
                    >
                      {item.status === 'active' ? '启用' : '停用'}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-xs text-neutral-600">{item.lastUsed}</td>
                  <td className="px-4 py-3">
                    <div className="flex items-center gap-1">
                      <button
                        onClick={() => handleViewDetail(item.id)}
                        className="p-1 text-neutral-500 hover:text-blue-600"
                        title="查看详情"
                        disabled={detailLoading}
                      >
                        <Eye size={16} />
                      </button>
                      <button
                        onClick={() => handleViewUsage(item.id)}
                        className="p-1 text-neutral-500 hover:text-violet-600"
                        title="使用统计"
                        disabled={usageLoading}
                      >
                        <BarChart3 size={16} />
                      </button>
                      <button
                        onClick={() => {
                          setSelectedItem(item)
                          setDialogType('edit')
                        }}
                        className="p-1 text-neutral-500 hover:text-amber-600"
                        title="编辑"
                      >
                        <Edit size={16} />
                      </button>
                      <button
                        onClick={() => {
                          setSelectedItem(item)
                          setDialogType('regenerate')
                        }}
                        className="p-1 text-neutral-500 hover:text-green-600"
                        title="重新生成"
                      >
                        <RefreshCw size={16} />
                      </button>
                      <button
                        onClick={() => handleToggleStatus(item.id, item.status === 'active')}
                        className={`p-1 text-neutral-500 ${
                          item.status === 'active' 
                            ? 'hover:text-red-600' 
                            : 'hover:text-green-600'
                        }`}
                        title={item.status === 'active' ? '禁用' : '启用'}
                        disabled={loading}
                      >
                        {item.status === 'active' ? (
                          <EyeOff size={16} />
                        ) : (
                          <Eye size={16} />
                        )}
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
                ))
              )}
            </tbody>
          </table>
        </div>
        
        {/* 分页组件 */}
        {totalPages > 1 && (
          <div className="flex items-center justify-between px-4 py-3 border-t border-neutral-200">
            <div className="text-sm text-neutral-600">
              显示 {(currentPage - 1) * pageSize + 1} - {Math.min(currentPage * pageSize, totalItems)} 条，共 {totalItems} 条记录
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
                {Array.from({ length: totalPages }, (_, i) => i + 1).map(page => (
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
                ))}
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
        <DialogPortal
          type={dialogType}
          selectedItem={selectedItem}
          usageData={usageData}
          keyDetail={keyDetail}
          usageLoading={usageLoading}
          detailLoading={detailLoading}
          onClose={() => {
            setDialogType(null)
            setSelectedItem(null)
            setUsageData(null)
            setKeyDetail(null)
          }}
          onAdd={handleAdd}
          onEdit={handleEdit}
          onDelete={handleDelete}
          onRegenerate={handleRegenerate}
        />
      )}
    </div>
  )
}

/** 对话框门户组件 */
const DialogPortal: React.FC<{
  type: DialogType
  selectedItem: ApiKey | null
  usageData?: any
  keyDetail?: UserServiceApiKeyDetail | null
  usageLoading?: boolean
  detailLoading?: boolean
  onClose: () => void
  onAdd: (item: Omit<ApiKey, 'id' | 'usage' | 'createdAt' | 'lastUsed' | 'keyValue'>) => void
  onEdit: (item: ApiKey) => void
  onDelete: () => void
  onRegenerate: (id: number) => void
}> = ({ 
  type, 
  selectedItem, 
  usageData, 
  keyDetail, 
  usageLoading, 
  detailLoading, 
  onClose, 
  onAdd, 
  onEdit, 
  onDelete, 
  onRegenerate 
}) => {
  if (!type) return null

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      {type === 'add' && <AddDialog onClose={onClose} onSubmit={onAdd} />}
      {type === 'edit' && selectedItem && <EditDialog item={selectedItem} onClose={onClose} onSubmit={onEdit} />}
      {type === 'delete' && selectedItem && <DeleteDialog item={selectedItem} onClose={onClose} onConfirm={onDelete} />}
      {type === 'stats' && selectedItem && <StatsDialog item={selectedItem} onClose={onClose} />}
      {type === 'usage' && selectedItem && <UsageDialog item={selectedItem} usageData={usageData} loading={usageLoading} onClose={onClose} />}
      {type === 'detail' && selectedItem && <DetailDialog item={selectedItem} keyDetail={keyDetail} loading={detailLoading} onClose={onClose} />}
      {type === 'regenerate' && selectedItem && <RegenerateDialog item={selectedItem} onClose={onClose} onConfirm={() => onRegenerate(selectedItem.id)} />}
    </div>
  )
}

/** 添加对话框 */
const AddDialog: React.FC<{
  onClose: () => void
  onSubmit: (item: Omit<ApiKey, 'id' | 'usage' | 'createdAt' | 'lastUsed' | 'keyValue'>) => void
}> = ({ onClose, onSubmit }) => {
  const [formData, setFormData] = useState({
    keyName: '',
    description: '',
    providerType: 'OpenAI',
    schedulingStrategy: 'round_robin' as 'round_robin' | 'priority' | 'weighted' | 'random',
    providerKeys: [] as string[],
    retryCount: 3,
    timeoutSeconds: 30,
    rateLimitPerMinute: 60,
    tokenLimitPerDay: 10000,
    status: 'active' as 'active' | 'disabled',
    limit: 10000,
  })

  // 可用的服务商类型
  const providerTypes = ['OpenAI', 'Anthropic', 'Google', 'Azure', 'Claude']
  
  // 调度策略选项
  const schedulingOptions = [
    { value: 'round_robin', label: '轮询调度' },
    { value: 'priority', label: '优先级调度' },
    { value: 'weighted', label: '权重调度' },
    { value: 'random', label: '随机调度' },
  ]

  // 模拟的账号API Keys
  const availableProviderKeys = [
    'openai-primary', 'openai-backup', 'openai-test',
    'claude-primary', 'claude-dev',
    'gemini-primary', 'gemini-test',
    'azure-primary', 'azure-backup'
  ]

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    onSubmit(formData)
  }

  // 处理数字输入框的增减
  const handleNumberChange = (field: string, delta: number) => {
    setFormData(prev => ({
      ...prev,
      [field]: Math.max(0, (prev[field as keyof typeof prev] as number) + delta)
    }))
  }

  return (
    <div className="bg-white rounded-2xl p-6 w-full max-w-2xl mx-4 max-h-[90vh] overflow-y-auto border border-neutral-200 hover:shadow-sm transition-shadow">
      <h3 className="text-lg font-medium text-neutral-900 mb-4">新增 API Key</h3>
      <form onSubmit={handleSubmit} className="space-y-4">
        {/* 服务名称 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">服务名称</label>
          <input
            type="text"
            required
            value={formData.keyName}
            onChange={(e) => setFormData({ ...formData, keyName: e.target.value })}
            placeholder="请输入服务名称"
            className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>

        {/* 描述 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">描述</label>
          <textarea
            value={formData.description}
            onChange={(e) => setFormData({ ...formData, description: e.target.value })}
            placeholder="请输入服务描述"
            rows={3}
            className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40 resize-none"
          />
        </div>

        {/* 服务商类型 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">服务商类型</label>
          <ModernSelect
            value={formData.providerType}
            onValueChange={(value) => setFormData({ ...formData, providerType: value })}
            options={providerTypes.map(type => ({
              value: type,
              label: type
            }))}
            placeholder="请选择服务商类型"
          />
        </div>

        {/* 调度策略 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">调度策略</label>
          <ModernSelect
            value={formData.schedulingStrategy}
            onValueChange={(value) => setFormData({ ...formData, schedulingStrategy: value as any })}
            options={schedulingOptions.map(option => ({
              value: option.value,
              label: option.label
            }))}
            placeholder="请选择调度策略"
          />
        </div>

        {/* 账号API Keys（多选） */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-2">账号API Keys</label>
          <MultiSelect
            value={formData.providerKeys}
            onValueChange={(value) => setFormData(prev => ({ ...prev, providerKeys: value }))}
            options={availableProviderKeys.map(key => ({
              value: key,
              label: key
            }))}
            placeholder="请选择账号API Keys"
            searchPlaceholder="搜索API Keys..."
            maxDisplay={3}
          />
        </div>

        {/* 数字配置选项 */}
        <div className="grid grid-cols-2 gap-4">
          {/* 重试次数 */}
          <div>
            <label className="block text-sm font-medium text-neutral-700 mb-1">重试次数</label>
            <div className="flex items-center">
              <button
                type="button"
                onClick={() => handleNumberChange('retryCount', -1)}
                className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
              >
                −
              </button>
              <input
                type="number"
                min="0"
                value={formData.retryCount}
                onChange={(e) => setFormData({ ...formData, retryCount: parseInt(e.target.value) || 0 })}
                className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
              />
              <button
                type="button"
                onClick={() => handleNumberChange('retryCount', 1)}
                className="px-3 py-2 border border-neutral-200 rounded-r-lg text-neutral-600 hover:bg-neutral-50"
              >
                +
              </button>
            </div>
          </div>

          {/* 超时时间 */}
          <div>
            <label className="block text-sm font-medium text-neutral-700 mb-1">超时时间(秒)</label>
            <div className="flex items-center">
              <button
                type="button"
                onClick={() => handleNumberChange('timeoutSeconds', -5)}
                className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
              >
                −
              </button>
              <input
                type="number"
                min="0"
                value={formData.timeoutSeconds}
                onChange={(e) => setFormData({ ...formData, timeoutSeconds: parseInt(e.target.value) || 0 })}
                className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
              />
              <button
                type="button"
                onClick={() => handleNumberChange('timeoutSeconds', 5)}
                className="px-3 py-2 border border-neutral-200 rounded-r-lg text-neutral-600 hover:bg-neutral-50"
              >
                +
              </button>
            </div>
          </div>

          {/* 速率限制 */}
          <div>
            <label className="block text-sm font-medium text-neutral-700 mb-1">速率限制/分钟</label>
            <div className="flex items-center">
              <button
                type="button"
                onClick={() => handleNumberChange('rateLimitPerMinute', -10)}
                className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
              >
                −
              </button>
              <input
                type="number"
                min="0"
                value={formData.rateLimitPerMinute}
                onChange={(e) => setFormData({ ...formData, rateLimitPerMinute: parseInt(e.target.value) || 0 })}
                className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
              />
              <button
                type="button"
                onClick={() => handleNumberChange('rateLimitPerMinute', 10)}
                className="px-3 py-2 border border-neutral-200 rounded-r-lg text-neutral-600 hover:bg-neutral-50"
              >
                +
              </button>
            </div>
          </div>

          {/* Token限制 */}
          <div>
            <label className="block text-sm font-medium text-neutral-700 mb-1">Token限制/天</label>
            <div className="flex items-center">
              <button
                type="button"
                onClick={() => handleNumberChange('tokenLimitPerDay', -1000)}
                className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
              >
                −
              </button>
              <input
                type="number"
                min="0"
                value={formData.tokenLimitPerDay}
                onChange={(e) => setFormData({ ...formData, tokenLimitPerDay: parseInt(e.target.value) || 0 })}
                className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
              />
              <button
                type="button"
                onClick={() => handleNumberChange('tokenLimitPerDay', 1000)}
                className="px-3 py-2 border border-neutral-200 rounded-r-lg text-neutral-600 hover:bg-neutral-50"
              >
                +
              </button>
            </div>
          </div>
        </div>

        {/* 使用限制 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">使用限制</label>
          <input
            type="number"
            required
            min="1"
            value={formData.limit}
            onChange={(e) => setFormData({ ...formData, limit: parseInt(e.target.value) || 1 })}
            className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>

        {/* 启用状态 */}
        <div className="flex items-center gap-3">
          <label className="text-sm font-medium text-neutral-700">启用状态</label>
          <button
            type="button"
            onClick={() => setFormData({ ...formData, status: formData.status === 'active' ? 'disabled' : 'active' })}
            className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
              formData.status === 'active' ? 'bg-violet-600' : 'bg-neutral-200'
            }`}
          >
            <span
              className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                formData.status === 'active' ? 'translate-x-6' : 'translate-x-1'
              }`}
            />
          </button>
          <span className="text-sm text-neutral-600">
            {formData.status === 'active' ? '启用' : '停用'}
          </span>
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

/** 编辑对话框 */
const EditDialog: React.FC<{
  item: ApiKey
  onClose: () => void
  onSubmit: (item: ApiKey) => void
}> = ({ item, onClose, onSubmit }) => {
  const [formData, setFormData] = useState({ ...item })

  // 可用的服务商类型
  const providerTypes = ['OpenAI', 'Anthropic', 'Google', 'Azure', 'Claude']
  
  // 调度策略选项
  const schedulingOptions = [
    { value: 'round_robin', label: '轮询调度' },
    { value: 'priority', label: '优先级调度' },
    { value: 'weighted', label: '权重调度' },
    { value: 'random', label: '随机调度' },
  ]

  // 模拟的账号API Keys
  const availableProviderKeys = [
    'openai-primary', 'openai-backup', 'openai-test',
    'claude-primary', 'claude-dev',
    'gemini-primary', 'gemini-test',
    'azure-primary', 'azure-backup'
  ]


  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    onSubmit(formData)
  }

  // 处理数字输入框的增减
  const handleNumberChange = (field: string, delta: number) => {
    setFormData(prev => ({
      ...prev,
      [field]: Math.max(0, (prev[field as keyof typeof prev] as number) + delta)
    }))
  }


  return (
    <div className="bg-white rounded-2xl p-6 w-full max-w-2xl mx-4 max-h-[90vh] overflow-y-auto border border-neutral-200 hover:shadow-sm transition-shadow">
      <h3 className="text-lg font-medium text-neutral-900 mb-4">编辑 API Key</h3>
      <form onSubmit={handleSubmit} className="space-y-4">
        {/* 服务名称 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">服务名称</label>
          <input
            type="text"
            required
            value={formData.keyName}
            onChange={(e) => setFormData({ ...formData, keyName: e.target.value })}
            className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>

        {/* 描述 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">描述</label>
          <textarea
            value={formData.description}
            onChange={(e) => setFormData({ ...formData, description: e.target.value })}
            rows={3}
            className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40 resize-none"
          />
        </div>

        {/* 服务商类型 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">服务商类型</label>
          <ModernSelect
            value={formData.providerType}
            onValueChange={(value) => setFormData({ ...formData, providerType: value })}
            options={providerTypes.map(type => ({
              value: type,
              label: type
            }))}
            placeholder="请选择服务商类型"
          />
        </div>

        {/* 调度策略 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">调度策略</label>
          <ModernSelect
            value={formData.schedulingStrategy}
            onValueChange={(value) => setFormData({ ...formData, schedulingStrategy: value as any })}
            options={schedulingOptions.map(option => ({
              value: option.value,
              label: option.label
            }))}
            placeholder="请选择调度策略"
          />
        </div>

        {/* 账号API Keys（多选） */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-2">账号API Keys</label>
          <MultiSelect
            value={formData.providerKeys}
            onValueChange={(value) => setFormData(prev => ({ ...prev, providerKeys: value }))}
            options={availableProviderKeys.map(key => ({
              value: key,
              label: key
            }))}
            placeholder="请选择账号API Keys"
            searchPlaceholder="搜索API Keys..."
            maxDisplay={3}
          />
        </div>

        {/* 数字配置选项 */}
        <div className="grid grid-cols-2 gap-4">
          {/* 重试次数 */}
          <div>
            <label className="block text-sm font-medium text-neutral-700 mb-1">重试次数</label>
            <div className="flex items-center">
              <button
                type="button"
                onClick={() => handleNumberChange('retryCount', -1)}
                className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
              >
                −
              </button>
              <input
                type="number"
                min="0"
                value={formData.retryCount}
                onChange={(e) => setFormData({ ...formData, retryCount: parseInt(e.target.value) || 0 })}
                className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
              />
              <button
                type="button"
                onClick={() => handleNumberChange('retryCount', 1)}
                className="px-3 py-2 border border-neutral-200 rounded-r-lg text-neutral-600 hover:bg-neutral-50"
              >
                +
              </button>
            </div>
          </div>

          {/* 超时时间 */}
          <div>
            <label className="block text-sm font-medium text-neutral-700 mb-1">超时时间(秒)</label>
            <div className="flex items-center">
              <button
                type="button"
                onClick={() => handleNumberChange('timeoutSeconds', -5)}
                className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
              >
                −
              </button>
              <input
                type="number"
                min="0"
                value={formData.timeoutSeconds}
                onChange={(e) => setFormData({ ...formData, timeoutSeconds: parseInt(e.target.value) || 0 })}
                className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
              />
              <button
                type="button"
                onClick={() => handleNumberChange('timeoutSeconds', 5)}
                className="px-3 py-2 border border-neutral-200 rounded-r-lg text-neutral-600 hover:bg-neutral-50"
              >
                +
              </button>
            </div>
          </div>

          {/* 速率限制 */}
          <div>
            <label className="block text-sm font-medium text-neutral-700 mb-1">速率限制/分钟</label>
            <div className="flex items-center">
              <button
                type="button"
                onClick={() => handleNumberChange('rateLimitPerMinute', -10)}
                className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
              >
                −
              </button>
              <input
                type="number"
                min="0"
                value={formData.rateLimitPerMinute}
                onChange={(e) => setFormData({ ...formData, rateLimitPerMinute: parseInt(e.target.value) || 0 })}
                className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
              />
              <button
                type="button"
                onClick={() => handleNumberChange('rateLimitPerMinute', 10)}
                className="px-3 py-2 border border-neutral-200 rounded-r-lg text-neutral-600 hover:bg-neutral-50"
              >
                +
              </button>
            </div>
          </div>

          {/* Token限制 */}
          <div>
            <label className="block text-sm font-medium text-neutral-700 mb-1">Token限制/天</label>
            <div className="flex items-center">
              <button
                type="button"
                onClick={() => handleNumberChange('tokenLimitPerDay', -1000)}
                className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
              >
                −
              </button>
              <input
                type="number"
                min="0"
                value={formData.tokenLimitPerDay}
                onChange={(e) => setFormData({ ...formData, tokenLimitPerDay: parseInt(e.target.value) || 0 })}
                className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
              />
              <button
                type="button"
                onClick={() => handleNumberChange('tokenLimitPerDay', 1000)}
                className="px-3 py-2 border border-neutral-200 rounded-r-lg text-neutral-600 hover:bg-neutral-50"
              >
                +
              </button>
            </div>
          </div>
        </div>

        {/* 使用限制 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">使用限制</label>
          <input
            type="number"
            required
            min="1"
            value={formData.limit}
            onChange={(e) => setFormData({ ...formData, limit: parseInt(e.target.value) || 1 })}
            className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>

        {/* 启用状态 */}
        <div className="flex items-center gap-3">
          <label className="text-sm font-medium text-neutral-700">启用状态</label>
          <button
            type="button"
            onClick={() => setFormData({ ...formData, status: formData.status === 'active' ? 'disabled' : 'active' })}
            className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
              formData.status === 'active' ? 'bg-violet-600' : 'bg-neutral-200'
            }`}
          >
            <span
              className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                formData.status === 'active' ? 'translate-x-6' : 'translate-x-1'
              }`}
            />
          </button>
          <span className="text-sm text-neutral-600">
            {formData.status === 'active' ? '启用' : '停用'}
          </span>
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
const DeleteDialog: React.FC<{
  item: ApiKey
  onClose: () => void
  onConfirm: () => void
}> = ({ item, onClose, onConfirm }) => {
  return (
    <div className="bg-white rounded-2xl p-6 w-full max-w-md mx-4 border border-neutral-200 hover:shadow-sm transition-shadow">
      <h3 className="text-lg font-medium text-neutral-900 mb-2">确认删除</h3>
      <p className="text-sm text-neutral-600 mb-4">
        确定要删除密钥 <strong>{item.keyName}</strong> 吗？此操作无法撤销。
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

/** 统计对话框 */
const StatsDialog: React.FC<{
  item: ApiKey
  onClose: () => void
}> = ({ item, onClose }) => {
  // 模拟统计数据
  const mockStats = {
    dailyUsage: [120, 150, 89, 245, 178, 234, 189],
    successRate: 98.5,
    avgResponseTime: 340,
    topEndpoints: [
      { endpoint: '/api/chat/completions', count: 1200 },
      { endpoint: '/api/embeddings', count: 320 },
    ]
  }

  return (
    <div className="bg-white rounded-2xl p-6 w-full max-w-2xl mx-4 max-h-[80vh] overflow-y-auto border border-neutral-200 hover:shadow-sm transition-shadow">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-lg font-medium text-neutral-900">API Key 统计</h3>
        <button
          onClick={onClose}
          className="text-neutral-500 hover:text-neutral-700"
        >
          ×
        </button>
      </div>
      
      <div className="space-y-6">
        {/* 基本信息 */}
        <div className="grid grid-cols-2 gap-4">
          <div className="p-4 bg-neutral-50 rounded-xl">
            <div className="text-sm text-neutral-600">密钥名称</div>
            <div className="font-medium">{item.keyName}</div>
          </div>
          <div className="p-4 bg-neutral-50 rounded-xl">
            <div className="text-sm text-neutral-600">服务商类型</div>
            <div className="font-medium">{item.providerType}</div>
          </div>
        </div>

        {/* 使用统计 */}
        <div className="grid grid-cols-3 gap-4">
          <div className="p-4 bg-violet-50 rounded-xl">
            <div className="text-sm text-violet-600">使用次数</div>
            <div className="text-2xl font-bold text-violet-900">{item.usage.toLocaleString()}</div>
          </div>
          <div className="p-4 bg-emerald-50 rounded-xl">
            <div className="text-sm text-emerald-600">成功率</div>
            <div className="text-2xl font-bold text-emerald-900">{mockStats.successRate}%</div>
          </div>
          <div className="p-4 bg-orange-50 rounded-xl">
            <div className="text-sm text-orange-600">平均响应时间</div>
            <div className="text-2xl font-bold text-orange-900">{mockStats.avgResponseTime}ms</div>
          </div>
        </div>

        {/* 使用趋势 */}
        <div>
          <h4 className="text-sm font-medium text-neutral-900 mb-3">7天使用趋势</h4>
          <div className="flex items-end gap-2 h-32">
            {mockStats.dailyUsage.map((value, index) => (
              <div key={index} className="flex-1 flex flex-col items-center">
                <div
                  className="w-full bg-violet-600 rounded-t"
                  style={{ height: `${(value / Math.max(...mockStats.dailyUsage)) * 100}%` }}
                />
                <div className="text-xs text-neutral-500 mt-1">{value}</div>
              </div>
            ))}
          </div>
        </div>

        {/* 热门接口 */}
        <div>
          <h4 className="text-sm font-medium text-neutral-900 mb-3">热门接口</h4>
          <div className="space-y-2">
            {mockStats.topEndpoints.map((endpoint, index) => (
              <div key={index} className="flex justify-between items-center py-2 border-b border-neutral-100">
                <code className="text-sm font-mono">{endpoint.endpoint}</code>
                <span className="text-sm text-neutral-600">{endpoint.count} 次</span>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  )
}

/** 重新生成API Key确认对话框 */
const RegenerateDialog: React.FC<{
  item: ApiKey
  onClose: () => void
  onConfirm: () => void
}> = ({ item, onClose, onConfirm }) => {
  return (
    <div className="bg-white rounded-2xl p-6 w-full max-w-md mx-4 border border-neutral-200 hover:shadow-sm transition-shadow">
      <h3 className="text-lg font-medium text-neutral-900 mb-2">重新生成API Key</h3>
      <p className="text-sm text-neutral-600 mb-4">
        确定要重新生成密钥 <strong>{item.keyName}</strong> 吗？
      </p>
      <div className="p-3 bg-amber-50 border border-amber-200 rounded-lg mb-4">
        <p className="text-sm text-amber-800">
          ⚠️ 重新生成后，旧的API Key将立即失效，请确保更新所有使用该密钥的应用。
        </p>
      </div>
      <div className="flex gap-3">
        <button
          onClick={onClose}
          className="flex-1 px-4 py-2 text-sm text-neutral-600 border border-neutral-200 rounded-lg hover:bg-neutral-50"
        >
          取消
        </button>
        <button
          onClick={onConfirm}
          className="flex-1 px-4 py-2 text-sm bg-amber-600 text-white rounded-lg hover:bg-amber-700"
        >
          重新生成
        </button>
      </div>
    </div>
  )
}

/** 使用统计对话框 */
const UsageDialog: React.FC<{
  item: ApiKey
  usageData: any
  loading?: boolean
  onClose: () => void
}> = ({ item, usageData, loading, onClose }) => {
  if (loading) {
    return (
      <div className="bg-white rounded-2xl p-6 w-full max-w-2xl mx-4 border border-neutral-200 hover:shadow-sm transition-shadow">
        <div className="flex items-center justify-center py-8">
          <RefreshCw size={24} className="animate-spin text-violet-600" />
          <span className="ml-2 text-neutral-600">加载使用统计...</span>
        </div>
      </div>
    )
  }

  if (!usageData) {
    return (
      <div className="bg-white rounded-2xl p-6 w-full max-w-md mx-4 border border-neutral-200 hover:shadow-sm transition-shadow">
        <h3 className="text-lg font-medium text-neutral-900 mb-4">使用统计</h3>
        <p className="text-sm text-neutral-600 text-center py-4">暂无使用数据</p>
        <button
          onClick={onClose}
          className="w-full px-4 py-2 text-sm text-neutral-600 border border-neutral-200 rounded-lg hover:bg-neutral-50"
        >
          关闭
        </button>
      </div>
    )
  }

  return (
    <div className="bg-white rounded-2xl p-6 w-full max-w-3xl mx-4 max-h-[80vh] overflow-y-auto border border-neutral-200 hover:shadow-sm transition-shadow">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-lg font-medium text-neutral-900">
          {item.keyName} - 使用统计
        </h3>
        <button
          onClick={onClose}
          className="text-neutral-500 hover:text-neutral-700"
        >
          <X size={20} />
        </button>
      </div>

      <div className="space-y-6">
        {/* 统计卡片 */}
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
          <div className="p-4 bg-blue-50 rounded-xl">
            <div className="text-sm text-blue-600">总请求数</div>
            <div className="text-2xl font-bold text-blue-900">{usageData.total_requests?.toLocaleString() || 0}</div>
          </div>
          <div className="p-4 bg-green-50 rounded-xl">
            <div className="text-sm text-green-600">成功请求</div>
            <div className="text-2xl font-bold text-green-900">{usageData.successful_requests?.toLocaleString() || 0}</div>
          </div>
          <div className="p-4 bg-red-50 rounded-xl">
            <div className="text-sm text-red-600">失败请求</div>
            <div className="text-2xl font-bold text-red-900">{usageData.failed_requests?.toLocaleString() || 0}</div>
          </div>
          <div className="p-4 bg-purple-50 rounded-xl">
            <div className="text-sm text-purple-600">成功率</div>
            <div className="text-2xl font-bold text-purple-900">{usageData.success_rate?.toFixed(1) || 0}%</div>
          </div>
        </div>

        {/* Token统计 */}
        <div>
          <h4 className="text-sm font-medium text-neutral-900 mb-3">Token使用统计</h4>
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-xs text-neutral-600">总Token</div>
              <div className="font-bold text-neutral-900">{usageData.total_tokens?.toLocaleString() || 0}</div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-xs text-neutral-600">输入Token</div>
              <div className="font-bold text-neutral-900">{usageData.tokens_prompt?.toLocaleString() || 0}</div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-xs text-neutral-600">输出Token</div>
              <div className="font-bold text-neutral-900">{usageData.tokens_completion?.toLocaleString() || 0}</div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-xs text-neutral-600">平均响应时间</div>
              <div className="font-bold text-neutral-900">{usageData.avg_response_time || 0}ms</div>
            </div>
          </div>
        </div>

        {/* 费用统计 */}
        {usageData.total_cost !== undefined && (
          <div>
            <h4 className="text-sm font-medium text-neutral-900 mb-3">费用统计</h4>
            <div className="p-4 bg-green-50 rounded-xl">
              <div className="text-sm text-green-600">总费用</div>
              <div className="text-xl font-bold text-green-900">
                ${usageData.total_cost?.toFixed(4) || '0.0000'} {usageData.cost_currency || 'USD'}
              </div>
            </div>
          </div>
        )}

        {/* 最后使用时间 */}
        {usageData.last_used && (
          <div>
            <h4 className="text-sm font-medium text-neutral-900 mb-2">最后使用</h4>
            <p className="text-sm text-neutral-600">
              {new Date(usageData.last_used).toLocaleString()}
            </p>
          </div>
        )}
      </div>
    </div>
  )
}

/** API Key详情对话框 */
const DetailDialog: React.FC<{
  item: ApiKey
  keyDetail: UserServiceApiKeyDetail | null
  loading?: boolean
  onClose: () => void
}> = ({ item, keyDetail, loading, onClose }) => {
  if (loading) {
    return (
      <div className="bg-white rounded-2xl p-6 w-full max-w-2xl mx-4 border border-neutral-200 hover:shadow-sm transition-shadow">
        <div className="flex items-center justify-center py-8">
          <RefreshCw size={24} className="animate-spin text-violet-600" />
          <span className="ml-2 text-neutral-600">加载详细信息...</span>
        </div>
      </div>
    )
  }

  const detail = keyDetail || {
    id: item.id,
    name: item.keyName,
    description: item.description,
    provider: item.providerType,
    api_key: item.keyValue,
    user_provider_keys_ids: item.providerKeys,
    scheduling_strategy: item.schedulingStrategy,
    retry_count: item.retryCount,
    timeout_seconds: item.timeoutSeconds,
    max_request_per_min: item.rateLimitPerMinute,
    max_tokens_per_day: item.tokenLimitPerDay,
    is_active: item.status === 'active',
    created_at: item.createdAt,
    updated_at: item.createdAt
  };

  return (
    <div className="bg-white rounded-2xl p-6 w-full max-w-2xl mx-4 max-h-[80vh] overflow-y-auto border border-neutral-200 hover:shadow-sm transition-shadow">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-lg font-medium text-neutral-900">API Key详情</h3>
        <button
          onClick={onClose}
          className="text-neutral-500 hover:text-neutral-700"
        >
          <X size={20} />
        </button>
      </div>

      <div className="space-y-6">
        {/* 基本信息 */}
        <div>
          <h4 className="text-sm font-medium text-neutral-900 mb-3">基本信息</h4>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-xs text-neutral-600 mb-1">密钥名称</div>
              <div className="font-medium">{detail.name}</div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-xs text-neutral-600 mb-1">服务商类型</div>
              <div className="font-medium">{detail.provider}</div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg col-span-1 md:col-span-2">
              <div className="text-xs text-neutral-600 mb-1">描述</div>
              <div className="font-medium">{detail.description || '无描述'}</div>
            </div>
          </div>
        </div>

        {/* API Key */}
        <div>
          <h4 className="text-sm font-medium text-neutral-900 mb-3">API Key</h4>
          <div className="p-3 bg-neutral-50 rounded-lg">
            <div className="flex items-center gap-2">
              <code className="flex-1 font-mono text-sm">{detail.api_key}</code>
              <button
                onClick={() => navigator.clipboard.writeText(detail.api_key)}
                className="p-1 text-neutral-500 hover:text-neutral-700"
                title="复制"
              >
                <Copy size={14} />
              </button>
            </div>
          </div>
        </div>

        {/* 配置信息 */}
        <div>
          <h4 className="text-sm font-medium text-neutral-900 mb-3">配置参数</h4>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-xs text-neutral-600 mb-1">调度策略</div>
              <div className="font-medium">{detail.scheduling_strategy || '轮询调度'}</div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-xs text-neutral-600 mb-1">重试次数</div>
              <div className="font-medium">{detail.retry_count || 3}</div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-xs text-neutral-600 mb-1">超时时间</div>
              <div className="font-medium">{detail.timeout_seconds || 30}秒</div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-xs text-neutral-600 mb-1">速率限制</div>
              <div className="font-medium">{detail.max_request_per_min || 60}/分钟</div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-xs text-neutral-600 mb-1">每日Token限制</div>
              <div className="font-medium">{detail.max_tokens_per_day?.toLocaleString() || '无限制'}</div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-xs text-neutral-600 mb-1">状态</div>
              <div className={`font-medium ${detail.is_active ? 'text-green-600' : 'text-red-600'}`}>
                {detail.is_active ? '启用' : '禁用'}
              </div>
            </div>
          </div>
        </div>

        {/* 关联的提供商密钥 */}
        {detail.user_provider_keys_ids && detail.user_provider_keys_ids.length > 0 && (
          <div>
            <h4 className="text-sm font-medium text-neutral-900 mb-3">关联的提供商密钥</h4>
            <div className="flex flex-wrap gap-2">
              {detail.user_provider_keys_ids.map((keyId, index) => (
                <span key={index} className="px-2 py-1 bg-violet-100 text-violet-700 rounded text-xs">
                  密钥ID: {keyId}
                </span>
              ))}
            </div>
          </div>
        )}

        {/* 时间信息 */}
        <div>
          <h4 className="text-sm font-medium text-neutral-900 mb-3">时间信息</h4>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-xs text-neutral-600 mb-1">创建时间</div>
              <div className="font-medium text-sm">
                {detail.created_at ? new Date(detail.created_at).toLocaleString() : item.createdAt}
              </div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-xs text-neutral-600 mb-1">更新时间</div>
              <div className="font-medium text-sm">
                {detail.updated_at ? new Date(detail.updated_at).toLocaleString() : item.createdAt}
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}

export default ApiUserKeysPage

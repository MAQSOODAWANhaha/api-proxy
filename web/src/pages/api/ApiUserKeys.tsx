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
import { api, UserServiceApiKey, ProviderType, SchedulingStrategy } from '../../lib/api'
import { createSafeStats, safeLargeNumber, safePercentage, safeResponseTime, safeCurrency, safeDateTime, safeTrendData } from '../../lib/dataValidation'
import {
  ResponsiveContainer,
  ComposedChart,
  Bar,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip as ReTooltip,
  Legend,
} from 'recharts'

// 使用API中定义的类型，并添加额外需要的字段
interface ApiKey extends UserServiceApiKey {
  scheduling_strategy?: string
  user_provider_keys_ids?: number[]
  retry_count?: number
  timeout_seconds?: number
  max_request_per_min?: number
  max_requests_per_day?: number
  max_tokens_per_day?: number
  max_cost_per_day?: number
}

// 服务商类型和调度策略从api.ts导入

/** 用户提供商密钥 */
interface UserProviderKey {
  id: number
  name: string
  display_name: string
}

/** 弹窗类型 */
type DialogType = 'add' | 'edit' | 'delete' | 'stats' | null

/** 页面主组件 */
const ApiUserKeysPage: React.FC = () => {
  const [data, setData] = useState<ApiKey[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [searchTerm, setSearchTerm] = useState('')
  const [statusFilter, setStatusFilter] = useState<'all' | 'active' | 'disabled'>('all')
  const [selectedItem, setSelectedItem] = useState<ApiKey | null>(null)
  const [dialogType, setDialogType] = useState<DialogType>(null)
  const [showKeyValues, setShowKeyValues] = useState<{ [key: string]: boolean }>({})
  
  // 分页状态
  const [currentPage, setCurrentPage] = useState(1)
  const [pageSize, setPageSize] = useState(10)
  const [totalItems, setTotalItems] = useState(0)

  // 初始化数据
  useEffect(() => {
    fetchData()
  }, [])

  // 获取API Keys列表
  const fetchData = async () => {
    setLoading(true)
    setError(null)
    
    try {
      const response = await api.userService.getKeys({
        page: currentPage,
        limit: pageSize,
        name: searchTerm || undefined,
        is_active: statusFilter === 'all' ? undefined : statusFilter === 'active'
      })
      
      if (response.success && response.data) {
        setData(response.data.service_api_keys || [])
        setTotalItems(response.data.pagination?.total || 0)
      } else {
        setError(response.message || '获取API Keys失败')
      }
    } catch (err) {
      setError('获取API Keys时发生错误')
      console.error('获取API Keys失败:', err)
    } finally {
      setLoading(false)
    }
  }

  // 过滤数据
  const filteredData = useMemo(() => {
    return data.filter((item) => {
      const matchesSearch = 
        item.name.toLowerCase().includes(searchTerm.toLowerCase()) ||
        (item.description && item.description.toLowerCase().includes(searchTerm.toLowerCase())) ||
        item.provider.toLowerCase().includes(searchTerm.toLowerCase())
      const matchesStatus = statusFilter === 'all' || 
        (statusFilter === 'active' && item.is_active) || 
        (statusFilter === 'disabled' && !item.is_active)
      return matchesSearch && matchesStatus
    })
  }, [data, searchTerm, statusFilter])

  // 分页数据和计算
  const paginatedData = useMemo(() => {
    const startIndex = (currentPage - 1) * pageSize
    return filteredData.slice(startIndex, startIndex + pageSize)
  }, [filteredData, currentPage, pageSize])

  const totalPages = Math.ceil(totalItems / pageSize)
  
  // 重置页码当过滤条件改变时
  useEffect(() => {
    setCurrentPage(1)
  }, [searchTerm, statusFilter])

  // 生成新的API Key
  const generateApiKey = () => {
    return 'sk-' + Math.random().toString(36).substring(2) + Math.random().toString(36).substring(2)
  }

  // 添加新API Key
  const handleAdd = async (newKey: Omit<ApiKey, 'id' | 'usage' | 'created_at' | 'last_used_at' | 'api_key'>) => {
    try {
      const response = await api.userService.createKey({
        name: newKey.name,
        description: newKey.description,
        provider_type_id: newKey.provider_type_id,
        user_provider_keys_ids: newKey.user_provider_keys_ids || [],
        scheduling_strategy: newKey.scheduling_strategy,
        retry_count: newKey.retry_count,
        timeout_seconds: newKey.timeout_seconds,
        max_request_per_min: newKey.max_request_per_min,
        max_requests_per_day: newKey.max_requests_per_day,
        max_tokens_per_day: newKey.max_tokens_per_day,
        max_cost_per_day: newKey.max_cost_per_day,
        expires_at: newKey.expires_at || undefined,
        is_active: newKey.is_active,
      })
      
      if (response.success) {
        // 重新加载数据
        fetchData()
        setDialogType(null)
      } else {
        setError(response.message || '创建API Key失败')
      }
    } catch (err) {
      setError('创建API Key时发生错误')
      console.error('创建API Key失败:', err)
    }
  }

  // 编辑API Key
  const handleEdit = async (updatedKey: ApiKey) => {
    try {
      const response = await api.userService.updateKey(updatedKey.id, {
        name: updatedKey.name,
        description: updatedKey.description,
        user_provider_keys_ids: updatedKey.user_provider_keys_ids,
        scheduling_strategy: updatedKey.scheduling_strategy,
        retry_count: updatedKey.retry_count,
        timeout_seconds: updatedKey.timeout_seconds,
        max_request_per_min: updatedKey.max_request_per_min,
        max_requests_per_day: updatedKey.max_requests_per_day,
        max_tokens_per_day: updatedKey.max_tokens_per_day,
        max_cost_per_day: updatedKey.max_cost_per_day,
        expires_at: updatedKey.expires_at || undefined,
      })
      
      if (response.success) {
        // 重新加载数据
        fetchData()
        setDialogType(null)
        setSelectedItem(null)
      } else {
        setError(response.message || '更新API Key失败')
      }
    } catch (err) {
      setError('更新API Key时发生错误')
      console.error('更新API Key失败:', err)
    }
  }

  // 删除API Key
  const handleDelete = async () => {
    if (selectedItem) {
      try {
        const response = await api.userService.deleteKey(selectedItem.id)
        
        if (response.success) {
          // 重新加载数据
          fetchData()
          setDialogType(null)
          setSelectedItem(null)
        } else {
          setError(response.message || '删除API Key失败')
        }
      } catch (err) {
        setError('删除API Key时发生错误')
        console.error('删除API Key失败:', err)
      }
    }
  }

  // 重新生成API Key
  const handleRegenerate = async (id: number) => {
    try {
      const response = await api.userService.regenerateKey(id)
      
      if (response.success) {
        // 重新加载数据
        fetchData()
      } else {
        setError(response.message || '重新生成API Key失败')
      }
    } catch (err) {
      setError('重新生成API Key时发生错误')
      console.error('重新生成API Key失败:', err)
    }
  }

  // 更新API Key状态
  const handleUpdateStatus = async (id: number, isActive: boolean) => {
    try {
      const response = await api.userService.updateKeyStatus(id, isActive)
      
      if (response.success) {
        // 重新加载数据
        fetchData()
      } else {
        setError(response.message || '更新API Key状态失败')
      }
    } catch (err) {
      setError('更新API Key状态时发生错误')
      console.error('更新API Key状态失败:', err)
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

  // 获取服务商显示名称 (这里暂时返回默认值，实际显示会在表格中处理)
  const getProviderDisplayName = (providerTypeId: number) => {
    return `服务商 ${providerTypeId}`
  }

  // 获取调度策略显示名称 (这里暂时返回原值，实际显示会在表格中处理)
  const getSchedulingStrategyLabel = (strategy: string) => {
    return strategy
  }

  // 获取提供商密钥显示名称 (这里暂时返回ID列表，实际显示会在表格中处理)
  const getProviderKeyDisplayNames = (keyIds: number[]) => {
    return keyIds.join(', ')
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
            onClick={fetchData}
            disabled={loading}
            className="flex items-center gap-2 px-3 py-2 text-sm text-neutral-600 hover:text-neutral-800 disabled:opacity-50"
            title="刷新数据"
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
        <div className="mb-4 p-4 bg-red-50 border border-red-200 rounded-lg text-red-700 text-sm">
          {error}
        </div>
      )}

      {/* 统计信息 */}
      <div className="mb-6 grid grid-cols-1 md:grid-cols-3 gap-4">
        <StatCard
          icon={<Key size={18} />}
          value={totalItems.toString()}
          label="总密钥数"
          color="#7c3aed"
        />
        <StatCard
          icon={<Activity size={18} />}
          value={data.filter(item => item.is_active).length.toString()}
          label="活跃密钥"
          color="#10b981"
        />
        <StatCard
          icon={<Users size={18} />}
          value={data.reduce((sum, item) => sum + (item.usage?.successful_requests || 0), 0).toLocaleString()}
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

      {/* 加载指示器 */}
      {loading && (
        <div className="flex justify-center py-8">
          <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-violet-600"></div>
        </div>
      )}

      {/* 数据表格 */}
      {!loading && (
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
                {paginatedData.map((item) => (
                  <tr key={item.id} className="text-neutral-800 hover:bg-neutral-50">
                    <td className="px-4 py-3">
                      <div>
                        <div className="font-medium">{item.name}</div>
                        <div className="text-xs text-neutral-500">
                          创建于 {new Date(item.created_at).toLocaleDateString()}
                        </div>
                      </div>
                    </td>
                    <td className="px-4 py-3">
                      <div className="max-w-xs truncate" title={item.description || ''}>
                        {item.description || '无描述'}
                      </div>
                    </td>
                    <td className="px-4 py-3">
                      <span className="px-2 py-1 bg-neutral-100 text-neutral-700 rounded text-xs font-medium">
                        {item.provider || `服务商 ${item.provider_type_id}`}
                      </span>
                    </td>
                    <td className="px-4 py-3">{renderMaskedKey(item.api_key, item.id)}</td>
                    <td className="px-4 py-3">
                      <div className="flex items-center gap-2">
                        <span className="text-sm">
                          {(item.usage?.successful_requests || 0).toLocaleString()} / {(item.usage?.failed_requests || 0).toLocaleString()}
                        </span>
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
                          style={{
                            width: `${Math.min(
                              ((item.usage?.successful_requests || 0) / Math.max(1, (item.usage?.successful_requests || 0) + (item.usage?.failed_requests || 0))) * 100,
                              100
                            )}%`
                          }}
                        />
                      </div>
                    </td>
                    <td className="px-4 py-3">
                      <span
                        className={`inline-flex items-center px-2 py-1 rounded-full text-xs font-medium ${
                          item.is_active
                            ? 'bg-emerald-50 text-emerald-700 ring-1 ring-emerald-200'
                            : 'bg-neutral-100 text-neutral-700 ring-1 ring-neutral-300'
                        }`}
                      >
                        {item.is_active ? '启用' : '停用'}
                      </span>
                    </td>
                    <td className="px-4 py-3 text-xs text-neutral-600">
                      {item.last_used_at 
                        ? new Date(item.last_used_at).toLocaleString() 
                        : '从未使用'}
                    </td>
                    <td className="px-4 py-3">
                      <div className="flex items-center gap-1">
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
                显示 {(currentPage - 1) * pageSize + 1} - {Math.min(currentPage * pageSize, totalItems)} 条，
                共 {totalItems} 条记录
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
                    {Array.from({ length: Math.min(5, totalPages) }, (_, i) => {
                      // 显示当前页附近5个页码
                      const start = Math.max(1, Math.min(currentPage - 2, totalPages - 4))
                      const page = start + i
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
      )}

      {/* 对话框组件 */}
      {dialogType && (
        <DialogPortal
          type={dialogType}
          selectedItem={selectedItem}
          onClose={() => {
            setDialogType(null)
            setSelectedItem(null)
          }}
          onAdd={handleAdd}
          onEdit={handleEdit}
          onDelete={handleDelete}
          onRegenerate={handleRegenerate}
          onUpdateStatus={handleUpdateStatus}
        />
      )}
    </div>
  )
}

/** 对话框门户组件 */
const DialogPortal: React.FC<{
  type: DialogType
  selectedItem: ApiKey | null
  onClose: () => void
  onAdd: (item: Omit<ApiKey, 'id' | 'usage' | 'created_at' | 'last_used_at' | 'api_key'>) => void
  onEdit: (item: ApiKey) => void
  onDelete: () => void
  onRegenerate: (id: number) => void
  onUpdateStatus: (id: number, isActive: boolean) => void
}> = ({ type, selectedItem, onClose, onAdd, onEdit, onDelete, onRegenerate, onUpdateStatus }) => {
  if (!type) return null

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      {type === 'add' && (
        <AddDialog
          onClose={onClose}
          onSubmit={onAdd}
        />
      )}
      {type === 'edit' && selectedItem && (
        <EditDialog
          item={selectedItem}
          onClose={onClose}
          onSubmit={onEdit}
        />
      )}
      {type === 'delete' && selectedItem && (
        <DeleteDialog
          item={selectedItem}
          onClose={onClose}
          onConfirm={onDelete}
        />
      )}
      {type === 'stats' && selectedItem && (
        <StatsDialog
          item={selectedItem}
          onClose={onClose}
        />
      )}
    </div>
  )
}

/** 添加对话框 */
const AddDialog: React.FC<{
  onClose: () => void
  onSubmit: (item: Omit<ApiKey, 'id' | 'usage' | 'created_at' | 'last_used_at' | 'api_key'>) => void
}> = ({ onClose, onSubmit }) => {
  const [formData, setFormData] = useState({
    name: '',
    description: '',
    provider: '', // 添加provider字段
    provider_type_id: 0, // 初始为0，表示未选择
    scheduling_strategy: '' as string,
    user_provider_keys_ids: [] as number[],
    retry_count: 3,
    timeout_seconds: 30,
    max_request_per_min: 60,
    max_requests_per_day: 50000,
    max_tokens_per_day: 10000,
    max_cost_per_day: 100.00,
    expires_at: '' as string | null,
    is_active: true,
  })

  // 弹窗独有的状态管理
  const [providerTypes, setProviderTypes] = useState<ProviderType[]>([])
  const [schedulingStrategies, setSchedulingStrategies] = useState<SchedulingStrategy[]>([])
  const [userProviderKeys, setUserProviderKeys] = useState<UserProviderKey[]>([])
  const [loadingProviderTypes, setLoadingProviderTypes] = useState(false)
  const [loadingSchedulingStrategies, setLoadingSchedulingStrategies] = useState(false)
  const [loadingKeys, setLoadingKeys] = useState(false)
  const [submitting, setSubmitting] = useState(false)
  

  // 获取服务商类型列表
  const fetchProviderTypesLocal = async () => {
    setLoadingProviderTypes(true)
    try {
      const response = await api.auth.getProviderTypes({ is_active: true })
      
      if (response.success && response.data) {
        setProviderTypes(response.data.provider_types || [])
        // 如果有可用的服务商类型，设置默认选择第一个
        if (response.data.provider_types && response.data.provider_types.length > 0) {
          const firstProvider = response.data.provider_types[0]
          setFormData(prev => ({ 
            ...prev, 
            provider_type_id: firstProvider.id,
            provider: firstProvider.name 
          }))
        }
      } else {
        console.error('[AddDialog] 获取服务商类型失败:', response.message)
      }
    } catch (err) {
      console.error('[AddDialog] 获取服务商类型异常:', err)
    } finally {
      setLoadingProviderTypes(false)
    }
  }

  // 获取调度策略列表
  const fetchSchedulingStrategiesLocal = async () => {
    setLoadingSchedulingStrategies(true)
    try {
      const response = await api.auth.getSchedulingStrategies()
      if (response.success && response.data) {
        setSchedulingStrategies(response.data.scheduling_strategies || [])
        // 设置默认调度策略
        const defaultStrategy = response.data.scheduling_strategies.find(s => s.is_default)
        if (defaultStrategy) {
          setFormData(prev => ({ ...prev, scheduling_strategy: defaultStrategy.value }))
        } else if (response.data.scheduling_strategies.length > 0) {
          setFormData(prev => ({ ...prev, scheduling_strategy: response.data!.scheduling_strategies[0].value }))
        }
      }
    } catch (err) {
      console.error('获取调度策略失败:', err)
    } finally {
      setLoadingSchedulingStrategies(false)
    }
  }

  // 获取用户提供商密钥列表的本地函数
  const fetchUserProviderKeysLocal = async (providerTypeId: number) => {
    if (!providerTypeId) {
      setUserProviderKeys([])
      return
    }
    
    setLoadingKeys(true)
    try {
      const response = await api.providerKeys.getSimpleList({ 
        is_active: true,
        provider_type_id: providerTypeId
      })
      if (response.success && response.data) {
        setUserProviderKeys(response.data.provider_keys.map(key => ({
          id: key.id,
          name: key.name,
          display_name: key.display_name
        })) || [])
      } else {
        setUserProviderKeys([])
      }
    } catch (err) {
      console.error('获取用户提供商密钥失败:', err)
      setUserProviderKeys([])
    } finally {
      setLoadingKeys(false)
    }
  }

  // 处理数字输入框的增减
  const handleNumberChange = (field: string, delta: number) => {
    setFormData(prev => ({
      ...prev,
      [field]: Math.max(0, (prev[field as keyof typeof prev] as number) + delta)
    }))
  }


  // 处理服务商类型变更
  const handleProviderTypeChange = (value: string) => {
    const selectedProvider = providerTypes.find(type => type.id.toString() === value)
    setFormData(prev => ({ 
      ...prev, 
      provider_type_id: parseInt(value),
      provider: selectedProvider ? selectedProvider.name : '',
      user_provider_keys_ids: [] // 重置选择的密钥
    }))
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (submitting) return
    
    setSubmitting(true)
    try {
      await onSubmit(formData)
    } catch (err) {
      console.error('提交失败:', err)
    } finally {
      setSubmitting(false)
    }
  }

  // 初始化：获取服务商类型和调度策略
  useEffect(() => {
    const initializeDialog = async () => {
      await Promise.all([
        fetchProviderTypesLocal(),
        fetchSchedulingStrategiesLocal()
      ])
    }
    initializeDialog()
  }, [])

  // 当服务商类型更改时，重新获取对应的用户提供商密钥
  useEffect(() => {
    if (formData.provider_type_id > 0) {
      fetchUserProviderKeysLocal(formData.provider_type_id)
      // 清空之前选择的密钥
      setFormData(prev => ({ ...prev, user_provider_keys_ids: [] }))
    }
  }, [formData.provider_type_id])

  return (
    <div className="bg-white rounded-2xl p-6 w-full max-w-2xl mx-4 max-h-[90vh] overflow-y-auto border border-neutral-200 hover:shadow-sm transition-shadow">
      <h3 className="text-lg font-medium text-neutral-900 mb-4">新增 API Key</h3>
      <form onSubmit={handleSubmit} className="space-y-4">
        {/* 服务名称 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">服务名称 *</label>
          <input
            type="text"
            required
            value={formData.name}
            onChange={(e) => setFormData({ ...formData, name: e.target.value })}
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
          <label className="block text-sm font-medium text-neutral-700 mb-1">服务商类型 *</label>
          
          {loadingProviderTypes ? (
            <div className="flex items-center gap-2 p-3 border border-neutral-200 rounded-lg">
              <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-violet-600"></div>
              <span className="text-sm text-neutral-600">加载服务商类型...</span>
            </div>
          ) : (
            <ModernSelect
              value={formData.provider_type_id > 0 ? formData.provider_type_id.toString() : ''}
              onValueChange={handleProviderTypeChange}
              options={providerTypes.map(type => ({
                value: type.id.toString(),
                label: type.display_name
              }))}
              placeholder="请选择服务商类型"
            />
          )}
        </div>


        {/* 调度策略 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">调度策略</label>
          {loadingSchedulingStrategies ? (
            <div className="flex items-center gap-2 p-3 border border-neutral-200 rounded-lg">
              <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-violet-600"></div>
              <span className="text-sm text-neutral-600">加载调度策略...</span>
            </div>
          ) : (
            <ModernSelect
              value={formData.scheduling_strategy}
              onValueChange={(value) => setFormData({ ...formData, scheduling_strategy: value })}
              options={schedulingStrategies.map(option => ({
                value: option.value,
                label: option.label
              }))}
              placeholder="请选择调度策略"
            />
          )}
        </div>

        {/* 账号API Keys（多选） */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-2">账号API Keys *</label>
          {loadingKeys ? (
            <div className="flex items-center gap-2 p-3 border border-neutral-200 rounded-lg">
              <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-violet-600"></div>
              <span className="text-sm text-neutral-600">加载密钥列表...</span>
            </div>
          ) : (
            <MultiSelect
              value={formData.user_provider_keys_ids.map(id => id.toString())}
              onValueChange={(value) => setFormData(prev => ({ 
                ...prev, 
                user_provider_keys_ids: value.map(v => parseInt(v)) 
              }))}
              options={userProviderKeys.map(key => ({
                value: key.id.toString(),
                label: key.display_name || key.name
              }))}
              placeholder="请选择账号API Keys"
              searchPlaceholder="搜索API Keys..."
              maxDisplay={3}
            />
          )}
          {!loadingKeys && userProviderKeys.length === 0 && (
            <p className="text-xs text-yellow-600 mt-1">当前服务商类型下没有可用的账号API Keys</p>
          )}
        </div>

        {/* 数字配置选项 */}
        <div className="grid grid-cols-2 gap-4">
          {/* 重试次数 */}
          <div>
            <label className="block text-sm font-medium text-neutral-700 mb-1">重试次数</label>
            <div className="flex items-center">
              <button
                type="button"
                onClick={() => handleNumberChange('retry_count', -1)}
                className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
              >
                −
              </button>
              <input
                type="number"
                min="0"
                value={formData.retry_count}
                onChange={(e) => setFormData({ ...formData, retry_count: parseInt(e.target.value) || 0 })}
                className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
              />
              <button
                type="button"
                onClick={() => handleNumberChange('retry_count', 1)}
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
                onClick={() => handleNumberChange('timeout_seconds', -5)}
                className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
              >
                −
              </button>
              <input
                type="number"
                min="0"
                value={formData.timeout_seconds}
                onChange={(e) => setFormData({ ...formData, timeout_seconds: parseInt(e.target.value) || 0 })}
                className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
              />
              <button
                type="button"
                onClick={() => handleNumberChange('timeout_seconds', 5)}
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
                onClick={() => handleNumberChange('max_request_per_min', -10)}
                className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
              >
                −
              </button>
              <input
                type="number"
                min="0"
                value={formData.max_request_per_min}
                onChange={(e) => setFormData({ ...formData, max_request_per_min: parseInt(e.target.value) || 0 })}
                className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
              />
              <button
                type="button"
                onClick={() => handleNumberChange('max_request_per_min', 10)}
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
                onClick={() => handleNumberChange('max_tokens_per_day', -1000)}
                className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
              >
                −
              </button>
              <input
                type="number"
                min="0"
                value={formData.max_tokens_per_day}
                onChange={(e) => setFormData({ ...formData, max_tokens_per_day: parseInt(e.target.value) || 0 })}
                className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
              />
              <button
                type="button"
                onClick={() => handleNumberChange('max_tokens_per_day', 1000)}
                className="px-3 py-2 border border-neutral-200 rounded-r-lg text-neutral-600 hover:bg-neutral-50"
              >
                +
              </button>
            </div>
          </div>
        </div>

        {/* 费用限制 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">费用限制/天 (USD)</label>
          <input
            type="number"
            step="0.01"
            min="0"
            value={formData.max_cost_per_day}
            onChange={(e) => setFormData({ ...formData, max_cost_per_day: parseFloat(e.target.value) || 0 })}
            className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>

        {/* 过期时间 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">过期时间</label>
          <input
            type="datetime-local"
            value={formData.expires_at || ''}
            onChange={(e) => setFormData({ ...formData, expires_at: e.target.value || null })}
            className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>

        {/* 启用状态 */}
        <div className="flex items-center gap-3">
          <label className="text-sm font-medium text-neutral-700">启用状态</label>
          <button
            type="button"
            onClick={() => setFormData({ ...formData, is_active: !formData.is_active })}
            className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
              formData.is_active ? 'bg-violet-600' : 'bg-neutral-200'
            }`}
          >
            <span
              className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                formData.is_active ? 'translate-x-6' : 'translate-x-1'
              }`}
            />
          </button>
          <span className="text-sm text-neutral-600">
            {formData.is_active ? '启用' : '停用'}
          </span>
        </div>

        <div className="flex gap-3 pt-4">
          <button
            type="button"
            onClick={onClose}
            disabled={submitting}
            className="flex-1 px-4 py-2 text-sm text-neutral-600 border border-neutral-200 rounded-lg hover:bg-neutral-50 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            取消
          </button>
          <button
            type="submit"
            disabled={submitting || loadingKeys}
            className="flex-1 px-4 py-2 text-sm bg-violet-600 text-white rounded-lg hover:bg-violet-700 disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
          >
            {submitting && (
              <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-white"></div>
            )}
            {submitting ? '创建中...' : '创建'}
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
  
  // 编辑弹窗独有的状态管理
  const [providerTypes, setProviderTypes] = useState<ProviderType[]>([])
  const [schedulingStrategies, setSchedulingStrategies] = useState<SchedulingStrategy[]>([])
  const [userProviderKeys, setUserProviderKeys] = useState<UserProviderKey[]>([])
  const [loadingProviderTypes, setLoadingProviderTypes] = useState(false)
  const [loadingSchedulingStrategies, setLoadingSchedulingStrategies] = useState(false)
  const [loadingKeys, setLoadingKeys] = useState(false)
  const [loadingDetail, setLoadingDetail] = useState(true)
  const [submitting, setSubmitting] = useState(false)

  // 获取API Key完整详情数据
  const fetchDetailData = async () => {
    setLoadingDetail(true)
    try {
      const response = await api.userService.getKeyDetail(item.id)
      if (response.success && response.data) {
        // 使用完整的详情数据更新formData
        setFormData({
          ...formData, // 保留原有的字段
          ...response.data,
          // 确保数字字段有默认值
          retry_count: response.data.retry_count || 0,
          timeout_seconds: response.data.timeout_seconds || 0,
          max_request_per_min: response.data.max_request_per_min || 0,
          max_requests_per_day: response.data.max_requests_per_day || 0,
          max_tokens_per_day: response.data.max_tokens_per_day || 0,
          max_cost_per_day: response.data.max_cost_per_day || 0,
          // 确保数组字段有默认值
          user_provider_keys_ids: response.data.user_provider_keys_ids || []
        })
      } else {
        console.error('获取API Key详情失败:', response.message)
      }
    } catch (err) {
      console.error('获取API Key详情异常:', err)
    } finally {
      setLoadingDetail(false)
    }
  }

  // 获取服务商类型列表
  const fetchProviderTypesLocal = async () => {
    setLoadingProviderTypes(true)
    try {
      const response = await api.auth.getProviderTypes({ is_active: true })
      
      if (response.success && response.data) {
        setProviderTypes(response.data.provider_types || [])
      } else {
        console.error('[EditDialog] 获取服务商类型失败:', response.message)
      }
    } catch (err) {
      console.error('[EditDialog] 获取服务商类型异常:', err)
    } finally {
      setLoadingProviderTypes(false)
    }
  }

  // 获取调度策略列表
  const fetchSchedulingStrategiesLocal = async () => {
    setLoadingSchedulingStrategies(true)
    try {
      const response = await api.auth.getSchedulingStrategies()
      if (response.success && response.data) {
        setSchedulingStrategies(response.data.scheduling_strategies || [])
      }
    } catch (err) {
      console.error('获取调度策略失败:', err)
    } finally {
      setLoadingSchedulingStrategies(false)
    }
  }

  // 获取用户提供商密钥列表的本地函数
  const fetchUserProviderKeysLocal = async (providerTypeId: number) => {
    if (!providerTypeId) {
      setUserProviderKeys([])
      return
    }
    
    setLoadingKeys(true)
    try {
      const response = await api.providerKeys.getSimpleList({ 
        is_active: true,
        provider_type_id: providerTypeId
      })
      if (response.success && response.data) {
        setUserProviderKeys(response.data.provider_keys.map(key => ({
          id: key.id,
          name: key.name,
          display_name: key.display_name
        })) || [])
      } else {
        setUserProviderKeys([])
      }
    } catch (err) {
      console.error('获取用户提供商密钥失败:', err)
      setUserProviderKeys([])
    } finally {
      setLoadingKeys(false)
    }
  }

  // 处理数字输入框的增减
  const handleNumberChange = (field: string, delta: number) => {
    setFormData(prev => ({
      ...prev,
      [field]: Math.max(0, (prev[field as keyof typeof prev] as number) + delta)
    }))
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (submitting) return
    
    setSubmitting(true)
    try {
      await onSubmit(formData)
    } catch (err) {
      console.error('提交失败:', err)
    } finally {
      setSubmitting(false)
    }
  }

  // 当服务商类型更改时，重新获取对应的用户提供商密钥
  useEffect(() => {
    if (formData.provider_type_id) {
      fetchUserProviderKeysLocal(formData.provider_type_id)
    }
  }, [formData.provider_type_id])

  // 初始化：获取完整详情数据、服务商类型、调度策略
  useEffect(() => {
    const initializeEditDialog = async () => {
      // 首先获取完整的详情数据
      await fetchDetailData()
      
      // 同时获取服务商类型和调度策略
      await Promise.all([
        fetchProviderTypesLocal(),
        fetchSchedulingStrategiesLocal()
      ])
    }
    initializeEditDialog()
  }, [])
  
  // 当详情数据加载完成且provider_type_id确定后，获取对应的用户提供商密钥
  useEffect(() => {
    if (!loadingDetail && formData.provider_type_id) {
      fetchUserProviderKeysLocal(formData.provider_type_id)
    }
  }, [loadingDetail, formData.provider_type_id])

  return (
    <div className="bg-white rounded-2xl p-6 w-full max-w-2xl mx-4 max-h-[90vh] overflow-y-auto border border-neutral-200 hover:shadow-sm transition-shadow">
      <h3 className="text-lg font-medium text-neutral-900 mb-4">编辑 API Key</h3>
      
      {loadingDetail ? (
        <div className="flex items-center justify-center py-8">
          <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-violet-600"></div>
          <span className="ml-2 text-neutral-600">正在加载详情数据...</span>
        </div>
      ) : (
        <form onSubmit={handleSubmit} className="space-y-4">
        {/* 服务名称 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">服务名称 *</label>
          <input
            type="text"
            required
            value={formData.name}
            onChange={(e) => setFormData({ ...formData, name: e.target.value })}
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
          <label className="block text-sm font-medium text-neutral-700 mb-1">服务商类型 *</label>
          <ModernSelect
            value={formData.provider_type_id.toString()}
            onValueChange={(value) => {
              const selectedProvider = providerTypes.find(type => type.id.toString() === value)
              setFormData({ 
                ...formData, 
                provider_type_id: parseInt(value),
                provider: selectedProvider ? selectedProvider.name : formData.provider
              })
            }}
            options={providerTypes.map(type => ({
              value: type.id.toString(),
              label: type.display_name
            }))}
            placeholder="请选择服务商类型"
          />
        </div>

        {/* 调度策略 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">调度策略</label>
          <ModernSelect
            value={formData.scheduling_strategy || ''}
            onValueChange={(value) => setFormData({ ...formData, scheduling_strategy: value })}
            options={schedulingStrategies.map(option => ({
              value: option.value,
              label: option.label
            }))}
            placeholder="请选择调度策略"
          />
        </div>

        {/* 账号API Keys（多选） */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-2">账号API Keys *</label>
          {loadingKeys ? (
            <div className="flex items-center gap-2 p-3 border border-neutral-200 rounded-lg">
              <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-violet-600"></div>
              <span className="text-sm text-neutral-600">加载密钥列表...</span>
            </div>
          ) : (
            <MultiSelect
              value={(formData.user_provider_keys_ids || []).map(id => id.toString())}
              onValueChange={(value) => setFormData(prev => ({ 
                ...prev, 
                user_provider_keys_ids: value.map(v => parseInt(v)) 
              }))}
              options={userProviderKeys.map(key => ({
                value: key.id.toString(),
                label: key.display_name || key.name
              }))}
              placeholder="请选择账号API Keys"
              searchPlaceholder="搜索API Keys..."
              maxDisplay={3}
            />
          )}
          {!loadingKeys && userProviderKeys.length === 0 && (
            <p className="text-xs text-yellow-600 mt-1">当前服务商类型下没有可用的账号API Keys</p>
          )}
        </div>

        {/* 数字配置选项 */}
        <div className="grid grid-cols-2 gap-4">
          {/* 重试次数 */}
          <div>
            <label className="block text-sm font-medium text-neutral-700 mb-1">重试次数</label>
            <div className="flex items-center">
              <button
                type="button"
                onClick={() => handleNumberChange('retry_count', -1)}
                className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
              >
                −
              </button>
              <input
                type="number"
                min="0"
                value={formData.retry_count}
                onChange={(e) => setFormData({ ...formData, retry_count: parseInt(e.target.value) || 0 })}
                className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
              />
              <button
                type="button"
                onClick={() => handleNumberChange('retry_count', 1)}
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
                onClick={() => handleNumberChange('timeout_seconds', -5)}
                className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
              >
                −
              </button>
              <input
                type="number"
                min="0"
                value={formData.timeout_seconds}
                onChange={(e) => setFormData({ ...formData, timeout_seconds: parseInt(e.target.value) || 0 })}
                className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
              />
              <button
                type="button"
                onClick={() => handleNumberChange('timeout_seconds', 5)}
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
                onClick={() => handleNumberChange('max_request_per_min', -10)}
                className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
              >
                −
              </button>
              <input
                type="number"
                min="0"
                value={formData.max_request_per_min}
                onChange={(e) => setFormData({ ...formData, max_request_per_min: parseInt(e.target.value) || 0 })}
                className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
              />
              <button
                type="button"
                onClick={() => handleNumberChange('max_request_per_min', 10)}
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
                onClick={() => handleNumberChange('max_tokens_per_day', -1000)}
                className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
              >
                −
              </button>
              <input
                type="number"
                min="0"
                value={formData.max_tokens_per_day}
                onChange={(e) => setFormData({ ...formData, max_tokens_per_day: parseInt(e.target.value) || 0 })}
                className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
              />
              <button
                type="button"
                onClick={() => handleNumberChange('max_tokens_per_day', 1000)}
                className="px-3 py-2 border border-neutral-200 rounded-r-lg text-neutral-600 hover:bg-neutral-50"
              >
                +
              </button>
            </div>
          </div>
        </div>

        {/* 费用限制 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">费用限制/天 (USD)</label>
          <input
            type="number"
            step="0.01"
            min="0"
            value={formData.max_cost_per_day}
            onChange={(e) => setFormData({ ...formData, max_cost_per_day: parseFloat(e.target.value) || 0 })}
            className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>

        {/* 过期时间 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">过期时间</label>
          <input
            type="datetime-local"
            value={formData.expires_at || ''}
            onChange={(e) => setFormData({ ...formData, expires_at: e.target.value || null })}
            className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>

        {/* 启用状态 */}
        <div className="flex items-center gap-3">
          <label className="text-sm font-medium text-neutral-700">启用状态</label>
          <button
            type="button"
            onClick={() => setFormData({ ...formData, is_active: !formData.is_active })}
            className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
              formData.is_active ? 'bg-violet-600' : 'bg-neutral-200'
            }`}
          >
            <span
              className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                formData.is_active ? 'translate-x-6' : 'translate-x-1'
              }`}
            />
          </button>
          <span className="text-sm text-neutral-600">
            {formData.is_active ? '启用' : '停用'}
          </span>
        </div>

        <div className="flex gap-3 pt-4">
          <button
            type="button"
            onClick={onClose}
            disabled={submitting}
            className="flex-1 px-4 py-2 text-sm text-neutral-600 border border-neutral-200 rounded-lg hover:bg-neutral-50 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            取消
          </button>
          <button
            type="submit"
            disabled={submitting || loadingKeys}
            className="flex-1 px-4 py-2 text-sm bg-violet-600 text-white rounded-lg hover:bg-violet-700 disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
          >
            {submitting && (
              <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-white"></div>
            )}
            {submitting ? '保存中...' : '保存'}
          </button>
        </div>
        </form>
      )}
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
        确定要删除密钥 <strong>{item.name}</strong> 吗？此操作无法撤销。
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
  // 使用数据验证工具创建安全的统计数据
  const usageStats = createSafeStats(item.usage)

  // 趋势数据状态管理
  const [trendData, setTrendData] = useState<any[]>([])
  const [trendLoading, setTrendLoading] = useState(true)
  const [detailedTrendData, setDetailedTrendData] = useState<any[]>([])
  const [detailedTrendLoading, setDetailedTrendLoading] = useState(true)

  // 获取趋势数据
  useEffect(() => {
    const fetchTrendData = async () => {
      try {
        setTrendLoading(true)
        const response = await api.userService.getKeyTrends(item.id, { days: 7 })
        if (
          response.success &&
          response.data &&
          Array.isArray(response.data.trend_data)
        ) {
          // 转换后端数据为前端需要的格式
          const formattedData = response.data.trend_data.map((point: any) =>
            Number(point?.requests ?? 0)
          )
          setTrendData(formattedData)
        } else {
          // 如果获取失败或数据格式不对，使用空数组
          setTrendData([])
        }
      } catch (error) {
        console.error('获取趋势数据失败:', error)
        setTrendData([])
      } finally {
        setTrendLoading(false)
      }
    }

    fetchTrendData()
  }, [item.id])

  // 获取7天的详细趋势数据（用于综合趋势图）
  useEffect(() => {
    const fetchDetailedTrendData = async () => {
      try {
        setDetailedTrendLoading(true)
        const response = await api.userService.getKeyTrends(item.id, { days: 7 })
        if (
          response.success &&
          response.data &&
          Array.isArray(response.data.trend_data)
        ) {
          // 转换为混合图表需要的格式
          const formattedData = response.data.trend_data.map((point: any) => ({
            date: point?.date,
            requests: Number(point?.requests ?? 0),
            tokens: Number(point?.tokens ?? point?.total_tokens ?? 0),
            successful_requests: Number(point?.successful_requests ?? 0),
            failed_requests: Number(point?.failed_requests ?? 0),
            cost: Number(point?.cost ?? point?.total_cost ?? 0),
            avg_response_time: Number(point?.avg_response_time ?? 0),
            success_rate: Number(point?.success_rate ?? 0),
          }))
          setDetailedTrendData(formattedData)
        } else {
          setDetailedTrendData([])
        }
      } catch (error) {
        console.error('获取详细趋势数据失败:', error)
        setDetailedTrendData([])
      } finally {
        setDetailedTrendLoading(false)
      }
    }

    fetchDetailedTrendData()
  }, [item.id])

  const stats = {
    ...usageStats,
    // 使用真实的趋势数据
    dailyUsage: trendData.length > 0 ? trendData : safeTrendData(),
  }

  const successRateDisplay = useMemo(
    () => safePercentage(stats.successRate).toFixed(2),
    [stats.successRate]
  )

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
            <div className="font-medium">{item.name}</div>
          </div>
          <div className="p-4 bg-neutral-50 rounded-xl">
            <div className="text-sm text-neutral-600">服务商类型</div>
            <div className="font-medium">{item.provider}</div>
          </div>
        </div>

        {/* 使用统计 */}
        <div className="grid grid-cols-4 gap-4">
          <div className="p-4 bg-violet-50 rounded-xl">
            <div className="text-sm text-violet-600">使用次数</div>
            <div className="text-2xl font-bold text-violet-900">
              {safeLargeNumber(stats.totalRequests)}
            </div>
          </div>
          <div className="p-4 bg-emerald-50 rounded-xl">
            <div className="text-sm text-emerald-600">成功率</div>
            <div className="text-2xl font-bold text-emerald-900">{successRateDisplay}%</div>
          </div>
          <div className="p-4 bg-orange-50 rounded-xl">
            <div className="text-sm text-orange-600">平均响应时间</div>
            <div className="text-2xl font-bold text-orange-900">{safeResponseTime(stats.avgResponseTime)}</div>
          </div>
          <div className="p-4 bg-blue-50 rounded-xl">
            <div className="text-sm text-blue-600">总花费</div>
            <div className="text-2xl font-bold text-blue-900">{safeCurrency(stats.totalCost)}</div>
          </div>
        </div>

        {/* 使用趋势 */}
        <div>
          <h4 className="text-sm font-medium text-neutral-900 mb-3">7天使用趋势</h4>
          <div className="flex items-end gap-2 h-32">
            {trendLoading ? (
              <div className="flex-1 flex items-center justify-center text-neutral-500">
                <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-violet-600"></div>
              </div>
            ) : (
              stats.dailyUsage.map((value, index) => (
                <div key={index} className="flex-1 flex flex-col items-center">
                  <div
                    className="w-full bg-violet-600 rounded-t"
                    style={{ height: `${(value / Math.max(...stats.dailyUsage, 1)) * 100}%` }}
                  />
                  <div className="text-xs text-neutral-500 mt-1">{value}</div>
                </div>
              ))
            )}
          </div>
        </div>

        {/* 7天综合趋势图（柱状图+折线图） */}
        <div>
          <h4 className="text-sm font-medium text-neutral-900 mb-3">7天综合趋势分析</h4>
          <div className="h-64 w-full">
            {detailedTrendLoading ? (
              <div className="flex items-center justify-center h-full text-neutral-500">
                <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-violet-600"></div>
              </div>
            ) : detailedTrendData.length > 0 ? (
              <ResponsiveContainer width="100%" height="100%">
                <ComposedChart
                  data={detailedTrendData}
                  margin={{ top: 20, right: 30, left: 20, bottom: 20 }}
                >
                  <CartesianGrid strokeDasharray="3 3" stroke="#E5E7EB" />
                  <XAxis
                    dataKey="date"
                    tickFormatter={(value) => {
                      const date = new Date(value)
                      return `${date.getMonth() + 1}/${date.getDate()}`
                    }}
                    tick={{ fontSize: 11, fill: '#6B7280' }}
                    axisLine={{ stroke: '#D1D5DB' }}
                    tickLine={{ stroke: '#D1D5DB' }}
                    height={40}
                    angle={-45}
                    dx={-8}
                    dy={8}
                  />
                  <YAxis
                    yAxisId="left"
                    tick={{ fontSize: 11, fill: '#6B7280' }}
                    axisLine={{ stroke: '#D1D5DB' }}
                    tickLine={{ stroke: '#D1D5DB' }}
                    width={40}
                  />
                  <YAxis
                    yAxisId="right"
                    orientation="right"
                    tick={{ fontSize: 11, fill: '#10B981' }}
                    axisLine={{ stroke: '#D1D5DB' }}
                    tickLine={{ stroke: '#D1D5DB' }}
                    width={50}
                  />
                  <ReTooltip
                    formatter={(value: any, name: any) => {
                      const labels: Record<string, string> = {
                        'requests': '请求数',
                        'tokens': 'Tokens',
                        'successful_requests': '成功请求',
                        'failed_requests': '失败请求',
                      }
                      return [`${value}`, labels[name] || name]
                    }}
                    labelFormatter={(label: any) => {
                      return `日期: ${label}`
                    }}
                    contentStyle={{ fontSize: 12 }}
                  />
                  <Legend
                    verticalAlign="top"
                    height={36}
                    iconType="circle"
                    iconSize={8}
                    wrapperStyle={{ fontSize: '11px' }}
                  />
                  {/* 柱状图：请求次数 */}
                  <Bar
                    yAxisId="left"
                    dataKey="requests"
                    fill="#6366F1"
                    name="请求数"
                    radius={[2, 2, 0, 0]}
                    barSize={12}
                  />
                  {/* 折线图：Token消耗 */}
                  <Line
                    yAxisId="right"
                    type="monotone"
                    dataKey="tokens"
                    stroke="#10B981"
                    strokeWidth={2}
                    name="Token消耗"
                    dot={{ fill: '#10B981', strokeWidth: 2, r: 3 }}
                    activeDot={{ r: 5 }}
                  />
                  {/* 成功请求率 */}
                  <Line
                    yAxisId="left"
                    type="monotone"
                    dataKey="successful_requests"
                    stroke="#059669"
                    strokeWidth={1.5}
                    name="成功请求"
                    dot={false}
                    strokeDasharray="3 3"
                  />
                </ComposedChart>
              </ResponsiveContainer>
            ) : (
              <div className="flex items-center justify-center h-full text-neutral-500">
                <div className="text-center">
                  <BarChart3 className="mx-auto h-12 w-12 text-neutral-400" />
                  <div className="mt-2 text-sm">暂无趋势数据</div>
                </div>
              </div>
            )}
          </div>
        </div>

        {/* 详细统计 */}
        <div className="grid grid-cols-2 gap-4">
          <div className="p-4 bg-neutral-50 rounded-xl">
            <div className="text-sm text-neutral-600">总Token数</div>
            <div className="text-2xl font-bold text-neutral-900">
              {safeLargeNumber(stats.totalTokens)}
            </div>
          </div>
          <div className="p-4 bg-neutral-50 rounded-xl">
            <div className="text-sm text-neutral-600">最后使用时间</div>
            <div className="text-lg font-medium text-neutral-900">
              {safeDateTime(stats.lastUsedAt)}
            </div>
          </div>
        </div>

              </div>
    </div>
  )
}

export default ApiUserKeysPage

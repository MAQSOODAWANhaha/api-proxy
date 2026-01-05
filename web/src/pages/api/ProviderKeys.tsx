/**
 * ProviderKeys.tsx
 * 账号（上游服务商）API Keys 管理页：完整的增删改查和统计功能
 */

import React, { useState, useMemo, useEffect, useCallback } from 'react'
import {
  Search,
  Plus,
  Edit,
  Trash2,
  Activity,
  BarChart3,
  Eye,
  EyeOff,
  RefreshCw,
  Copy,
  Shield,
  ChevronLeft,
  ChevronRight,
  DollarSign,
} from 'lucide-react'
import { StatCard } from '../../components/common/StatCard'
import FilterSelect from '../../components/common/FilterSelect'
import HealthStatusDetail from '../../components/provider/HealthStatusDetail'
import { LoadingSpinner, LoadingState } from '@/components/ui/loading'
import { Skeleton } from '@/components/ui/skeleton'
import {
  api,
  CreateProviderKeyRequest,
  ProviderKey,
  ProviderKeysDashboardStatsResponse,
  ProviderType,
  UpdateProviderKeyRequest,
} from '../../lib/api'
import { toast } from 'sonner'
import DialogPortal from './provider-keys/dialogs/DialogPortal'
import { DialogType, LocalProviderKey, ProviderKeyEditFormState, ProviderKeyFormState } from './provider-keys/types'
import { copyWithFeedback } from '../../lib/clipboard'


// 数据转换工具函数
const transformProviderKeyFromAPI = (apiKey: ProviderKey): LocalProviderKey => {
  return {
    ...apiKey,
    keyName: apiKey.name,
    keyValue: apiKey.api_key,
    status: apiKey.is_active ? 'active' : 'disabled',
    createdAt: apiKey.created_at,
    requestLimitPerMinute: apiKey.max_requests_per_minute,
    tokenLimitPromptPerMinute: apiKey.max_tokens_prompt_per_minute,
    requestLimitPerDay: apiKey.max_requests_per_day,
    healthStatus: apiKey.status?.health_status || apiKey.health_status || 'unknown',
    // 添加缺失的字段，从usage中获取
    cost: apiKey.usage?.total_cost || 0,
    usage: apiKey.usage || {
      total_requests: 0,
      successful_requests: 0,
      failed_requests: 0,
      success_rate: 0,
      total_tokens: 0,
      total_cost: 0,
      avg_response_time: 0
    },
    // 添加限流剩余时间（可选字段）
    rateLimitRemainingSeconds: apiKey.status?.rate_limit_remaining_seconds,
    // 添加健康状态详情
    health_status_detail: apiKey.health_status_detail,
  }
}

/** 页面主组件 */
const ProviderKeysPage: React.FC = () => {
  // 数据状态
  const [data, setData] = useState<LocalProviderKey[]>([])
  const [dashboardStats, setDashboardStats] = useState<ProviderKeysDashboardStatsResponse | null>(null)
  const [loading, setLoading] = useState(true)
  const [statsLoading, setStatsLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  
  // UI状态
  const [searchTerm, setSearchTerm] = useState('')
  const [statusFilter, setStatusFilter] = useState<'all' | 'healthy' | 'rate_limited' | 'unhealthy'>('all')
  const [providerFilter, setProviderFilter] = useState<string>('all')
  const [selectedItem, setSelectedItem] = useState<LocalProviderKey | null>(null)
  const [dialogType, setDialogType] = useState<DialogType>(null)
  const [showKeyValues, setShowKeyValues] = useState<{ [key: string]: boolean }>({})
  
  // 分页状态
  const [currentPage, setCurrentPage] = useState(1)
  const [pageSize, setPageSize] = useState(10)
  const [totalItems, setTotalItems] = useState(0)
  const [totalPages, setTotalPages] = useState(0)

  // 获取仪表板统计数据
  const fetchDashboardStats = async () => {
    setStatsLoading(true)
    try {
      const response = await api.providerKeys.getDashboardStats()
      if (response.success && response.data) {
        setDashboardStats(response.data)
      } else {
        console.error('获取仪表板统计数据失败:', response.error?.message)
      }
    } catch (error) {
      console.error('获取仪表板统计数据异常:', error)
    } finally {
      setStatsLoading(false)
    }
  }

  // 获取密钥列表数据
  const fetchData = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const response = await api.providerKeys.getList({
        page: currentPage,
        limit: pageSize,
        search: searchTerm || undefined,
        provider: providerFilter === 'all' ? undefined : providerFilter,
        // 当选择"全部状态"时不传递status参数，因为后端枚举不包含"all"
        ...(statusFilter !== 'all' && { status: statusFilter }),
      })

      if (response.success && response.data) {
        const transformedData = response.data.provider_keys.map(transformProviderKeyFromAPI)
        setData(transformedData)
        setTotalItems(response.data.pagination.total)
        setTotalPages(response.data.pagination.pages)
      } else {
        throw new Error(response.error?.message || '获取密钥列表失败')
      }
    } catch (error) {
      console.error('获取密钥列表失败:', error)
      const msg = error instanceof Error ? error.message : '获取密钥列表失败'
      setError(msg)
      setData([])
    } finally {
      setLoading(false)
    }
  }, [currentPage, pageSize, searchTerm, statusFilter, providerFilter])

  // 初始化数据加载
  useEffect(() => {
    fetchDashboardStats()
  }, [])

  useEffect(() => {
    fetchData()
  }, [fetchData])

  // 获取所有账号列表
  const providers = useMemo(() => {
    const uniqueProviders = Array.from(new Set(data.map(item => item.provider)))
    return uniqueProviders
  }, [data])

  // 由于后端已经处理了过滤和分页，前端直接使用返回的数据
  const paginatedData = data
  const pageLoading = loading || statsLoading
  
  // 重置页码当过滤条件改变时
  React.useEffect(() => {
    setCurrentPage(1)
  }, [searchTerm, statusFilter, providerFilter])

  // 生成新的API Key
  const generateApiKey = (provider: string) => {
    const prefixes: { [key: string]: string } = {
      'OpenAI': 'sk-',
      'Anthropic': 'sk-ant-',
      'Google': 'AIzaSy',
      'Azure': 'azure-',
    }
    const prefix = prefixes[provider] || 'key-'
    return prefix + Math.random().toString(36).substring(2) + Math.random().toString(36).substring(2)
  }

  // 添加新API Key
  const handleAdd = async (newKey: ProviderKeyFormState) => {
    try {
      // 找到对应的provider_type_id
      const providerTypesResponse = await api.auth.getProviderTypes({ is_active: true })
      let providerTypeId = newKey.provider_type_id || Number(newKey.provider) || 0
      let matchedType: ProviderType | undefined

      if (providerTypesResponse.success && providerTypesResponse.data?.provider_types) {
        matchedType = providerTypesResponse.data.provider_types.find(
          type => type.id === providerTypeId
        )
        if (!matchedType && newKey.provider) {
          matchedType = providerTypesResponse.data.provider_types.find(
            type => type.display_name === newKey.provider || type.name === newKey.provider
          )
        }
        if (matchedType && !providerTypeId) {
          providerTypeId = matchedType.id
        }
      }

      if (!providerTypeId) {
        throw new Error('未选择有效的服务商类型')
      }

      const authType = matchedType?.auth_type || newKey.auth_type || 'api_key'
      const providerLabel = matchedType?.display_name || matchedType?.name || newKey.provider

      const payload: CreateProviderKeyRequest = {
        provider_type_id: providerTypeId,
        name: newKey.keyName,
        api_key: newKey.keyValue || generateApiKey(providerLabel),
        auth_type: authType,
        weight: newKey.weight || 1,
        max_requests_per_minute: newKey.requestLimitPerMinute || 0,
        max_tokens_prompt_per_minute: newKey.tokenLimitPromptPerMinute || 0,
        max_requests_per_day: newKey.requestLimitPerDay || 0,
        is_active: newKey.status === 'active',
      }

      const projectId = (newKey.project_id || '').trim()
      if (projectId && matchedType?.name === 'gemini' && authType.includes('oauth')) {
        payload.project_id = projectId
      }

      const response = await api.providerKeys.create(payload)

      if (response.success) {
        // 刷新数据
        await fetchData()
        await fetchDashboardStats()
        setDialogType(null)
        toast.success('创建密钥成功')
      } else {
        throw new Error(response.error?.message || '创建密钥失败')
      }
    } catch (error) {
      console.error('添加密钥失败:', error)
      const msg = error instanceof Error ? error.message : '添加密钥失败'
      toast.error(msg)
    }
  }

  // 编辑API Key
  const handleEdit = async (updatedKey: ProviderKeyEditFormState) => {
    try {
      // 找到对应的provider_type_id
      const providerTypesResponse = await api.auth.getProviderTypes({ is_active: true })
      let providerTypeId = updatedKey.provider_type_id || Number(updatedKey.provider) || 0
      let matchedType: ProviderType | undefined

      if (providerTypesResponse.success && providerTypesResponse.data?.provider_types) {
        matchedType = providerTypesResponse.data.provider_types.find(
          type => type.id === providerTypeId
        )
        if (!matchedType && updatedKey.provider) {
          matchedType = providerTypesResponse.data.provider_types.find(
            type => type.display_name === updatedKey.provider || type.name === updatedKey.provider
          )
        }
        if (matchedType && !providerTypeId) {
          providerTypeId = matchedType.id
        }
      }

      if (!providerTypeId) {
        throw new Error('未选择有效的服务商类型')
      }

      const authType = matchedType?.auth_type || updatedKey.auth_type || 'api_key'

      const payload: UpdateProviderKeyRequest = {
        provider_type_id: providerTypeId,
        name: updatedKey.keyName,
        api_key: updatedKey.keyValue,
        auth_type: authType,
        weight: updatedKey.weight,
        max_requests_per_minute: updatedKey.requestLimitPerMinute,
        max_tokens_prompt_per_minute: updatedKey.tokenLimitPromptPerMinute,
        max_requests_per_day: updatedKey.requestLimitPerDay,
        is_active: updatedKey.status === 'active',
      }

      const projectId = (updatedKey.project_id || '').trim()
      if (projectId && matchedType?.name === 'gemini' && authType.includes('oauth')) {
        payload.project_id = projectId
      }

      const keyId = updatedKey.id ?? selectedItem?.id
      if (!keyId) {
        throw new Error('当前密钥缺少ID，无法提交更新')
      }

      const response = await api.providerKeys.update(String(keyId), payload)

      if (response.success) {
        // 刷新数据
        await fetchData()
        await fetchDashboardStats()
        setDialogType(null)
        setSelectedItem(null)
        toast.success('更新密钥成功')
      } else {
        throw new Error(response.error?.message || '更新密钥失败')
      }
    } catch (error) {
      console.error('编辑密钥失败:', error)
      const msg = error instanceof Error ? error.message : '编辑密钥失败'
      toast.error(msg)
    }
  }

  // 删除API Key
  const handleDelete = async () => {
    if (selectedItem) {
      try {
        const response = await api.providerKeys.delete(String(selectedItem.id))
        
        if (response.success) {
          // 刷新数据
          await fetchData()
          await fetchDashboardStats()
          setDialogType(null)
          setSelectedItem(null)
          toast.success('删除密钥成功')
        } else {
          throw new Error(response.error?.message || '删除密钥失败')
        }
      } catch (error) {
        console.error('删除密钥失败:', error)
        const msg = error instanceof Error ? error.message : '删除密钥失败'
        toast.error(msg)
      }
    }
  }

  // 健康检查
  const performHealthCheck = async (id: string) => {
    try {
      const response = await api.providerKeys.healthCheck(id)
      
      if (response.success) {
        // 更新本地数据中的健康状态
        const newHealthStatus = response.data?.health_status || 'healthy'
        setData(data.map(item =>
          String(item.id) === String(id)
            ? { ...item, healthStatus: newHealthStatus }
            : item
        ))
      } else {
        throw new Error(response.error?.message || '健康检查失败')
      }
    } catch (error) {
      console.error('健康检查失败:', error)
      // 显示错误状态
      setData(data.map(item =>
        String(item.id) === String(id)
          ? { ...item, healthStatus: 'error' }
          : item
      ))
    }
  }

  // 切换API Key可见性
  const toggleKeyVisibility = (id: string) => {
    setShowKeyValues(prev => ({ ...prev, [id]: !prev[id] }))
  }

  const normalizeHealthStatus = (
    status: string
  ): 'healthy' | 'rate_limited' | 'unhealthy' => {
    if (status === 'healthy' || status === 'rate_limited' || status === 'unhealthy') {
      return status
    }
    return 'unhealthy'
  }

  // 渲染遮罩的API Key
  const renderMaskedKey = (key: string, id: string) => {
    const isVisible = showKeyValues[id]
    return (
      <div className="flex items-center gap-2">
        <code className="font-mono text-xs bg-neutral-100 px-2 py-1 rounded">
          {isVisible ? key : `${key.substring(0, 12)}...${key.substring(key.length - 4)}`}
        </code>
        <button
          onClick={() => toggleKeyVisibility(id)}
          className="text-neutral-500 hover:text-neutral-700"
          title={isVisible ? '隐藏' : '显示'}
          aria-label={isVisible ? '隐藏 API Key' : '显示 API Key'}
        >
          {isVisible ? <EyeOff size={14} /> : <Eye size={14} />}
        </button>
        <button
          onClick={() => void copyWithFeedback(key, 'API Key')}
          className="text-neutral-500 hover:text-neutral-700"
          title="复制 API Key"
          aria-label="复制 API Key"
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
          <h2 className="text-lg font-medium text-neutral-800">账号API Keys</h2>
          <p className="text-sm text-neutral-600 mt-1">管理上游账号的API访问密钥</p>
        </div>
        <div className="flex gap-2">
          <button
            onClick={() => {
              fetchData()
              fetchDashboardStats()
            }}
            disabled={pageLoading}
            className="flex items-center gap-2 px-3 py-2 text-sm text-neutral-600 hover:text-neutral-800 disabled:opacity-50 disabled:cursor-not-allowed"
            title="刷新数据"
          >
            {pageLoading ? <LoadingSpinner size="sm" tone="muted" /> : <RefreshCw size={16} />}
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
            icon={<Shield size={18} />}
            value={dashboardStats?.total_keys?.toString() || '0'}
            label="总密钥数"
            color="#7c3aed"
          />
          <StatCard
            icon={<Activity size={18} />}
            value={dashboardStats?.active_keys?.toString() || '0'}
            label="活跃密钥"
            color="#10b981"
          />
          <StatCard
            icon={<BarChart3 size={18} />}
            value={dashboardStats?.total_usage?.toLocaleString() || '0'}
            label="总使用次数"
            color="#0ea5e9"
          />
          <StatCard
            icon={<DollarSign size={18} />}
            value={`$${dashboardStats?.total_cost?.toFixed(2) || '0.00'}`}
            label="总花费"
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
            placeholder="搜索账号、密钥名称..."
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            className="w-full pl-10 pr-4 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>
        <div className="flex items-center gap-4">
          <FilterSelect
            value={providerFilter}
            onValueChange={setProviderFilter}
            options={[
              { value: 'all', label: '全部账号' },
              ...providers.map(provider => ({
                value: provider,
                label: provider
              }))
            ]}
            placeholder="全部账号"
          />
          <FilterSelect
            value={statusFilter}
            onValueChange={(value) => setStatusFilter(value as 'all' | 'healthy' | 'rate_limited' | 'unhealthy')}
            options={[
              { value: 'all', label: '全部状态' },
              { value: 'healthy', label: '健康' },
              { value: 'rate_limited', label: '限流中' },
              { value: 'unhealthy', label: '异常' }
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
                <th className="px-4 py-3 text-left font-medium">账号</th>
                <th className="px-4 py-3 text-left font-medium">密钥名称</th>
                <th className="px-4 py-3 text-left font-medium">API Key</th>
                <th className="px-4 py-3 text-left font-medium">使用情况</th>
                <th className="px-4 py-3 text-left font-medium">花费</th>
                <th className="px-4 py-3 text-left font-medium">健康状态</th>
                <th className="px-4 py-3 text-left font-medium">权重</th>
                <th className="px-4 py-3 text-left font-medium">操作</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-neutral-200">
              {pageLoading ? (
                <tr>
                  <td colSpan={8} className="px-4 py-10 text-center">
                    <div className="flex justify-center">
                      <LoadingState text="加载中..." />
                    </div>
                  </td>
                </tr>
              ) : paginatedData.length === 0 ? (
                <tr>
                  <td colSpan={8} className="px-4 py-10 text-center text-neutral-500">
                    暂无数据
                  </td>
                </tr>
              ) : (
                paginatedData.map((item) => (
                <tr key={item.id} className="text-neutral-800 hover:bg-neutral-50">
                  <td className="px-4 py-3">
                    <div>
                      <div className="font-medium flex items-center gap-2">
                        <Shield size={16} className="text-neutral-500" />
                        {item.provider}
                      </div>
                    </div>
                  </td>
                  <td className="px-4 py-3">
                    <div>
                      <div className="font-medium">{item.keyName}</div>
                      <div className="text-xs text-neutral-500">创建于 {item.createdAt}</div>
                    </div>
                  </td>
                  <td className="px-4 py-3">{renderMaskedKey(item.keyValue, String(item.id))}</td>
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
                    <div className="text-xs text-neutral-500 mt-1">
                      请求限制: {item.requestLimitPerMinute ? `${item.requestLimitPerMinute}/分钟` : '无'}
                    </div>
                    <div className="text-xs text-neutral-500">
                      Token限制: {item.tokenLimitPromptPerMinute ? `${item.tokenLimitPromptPerMinute.toLocaleString()}/分钟` : '无'}
                    </div>
                    <div className="text-xs text-neutral-500">
                      请求限制: {item.requestLimitPerDay ? `${item.requestLimitPerDay.toLocaleString()}/天` : '无'}
                    </div>
                  </td>
                  <td className="px-4 py-3">
                    <div className="text-sm font-medium text-neutral-900">${(item.cost || 0).toFixed(2)}</div>
                    <div className="text-xs text-neutral-500">本月花费</div>
                  </td>
                  <td className="px-4 py-3">
                    <div className="flex items-center gap-2">
                      <HealthStatusDetail
                        health_status_detail={item.health_status_detail}
                        health_status={normalizeHealthStatus(item.healthStatus)}
                      />
                      <button
                        onClick={() => performHealthCheck(String(item.id))}
                        className="text-neutral-500 hover:text-neutral-700"
                        title="健康检查"
                      >
                        <RefreshCw size={12} />
                      </button>
                    </div>
                  </td>
                  <td className="px-4 py-3">
                    <span className="inline-flex items-center px-2 py-1 rounded-full text-xs font-medium bg-blue-50 text-blue-700 ring-1 ring-blue-200">
                      权重 {item.weight}
                    </span>
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
                <select
                  className="h-8 rounded-md border border-neutral-200 bg-white px-2 text-sm"
                  value={pageSize}
                  onChange={(e) => {
                    const newSize = Number(e.target.value)
                    setPageSize(newSize)
                    setCurrentPage(1) // 重置到第一页
                  }}
                >
                  <option value={10}>10</option>
                  <option value={20}>20</option>
                  <option value={50}>50</option>
                  <option value={100}>100</option>
                </select>
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
          onClose={() => {
            setDialogType(null)
            setSelectedItem(null)
          }}
          onAdd={handleAdd}
          onEdit={handleEdit}
          onDelete={handleDelete}
          onRefresh={async () => {
            await fetchData()
            await fetchDashboardStats()
          }}
        />
      )}
    </div>
  )
}


export default ProviderKeysPage

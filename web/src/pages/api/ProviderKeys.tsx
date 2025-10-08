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
  BarChart3,
  Eye,
  EyeOff,
  Filter,
  RefreshCw,
  Copy,
  Shield,
  Activity,
  ChevronLeft,
  ChevronRight,
  DollarSign,
} from 'lucide-react'
import { StatCard } from '../../components/common/StatCard'
import FilterSelect from '../../components/common/FilterSelect'
import ModernSelect from '../../components/common/ModernSelect'
import AuthTypeSelector from '../../components/common/AuthTypeSelector'
import OAuthHandler, { OAuthStatus, OAuthResult } from '../../components/common/OAuthHandler'
import { api, CreateProviderKeyRequest, ProviderKey, ProviderKeysDashboardStatsResponse, ProviderKeysListResponse, ProviderType, UpdateProviderKeyRequest } from '../../lib/api'
import { toast } from 'sonner'
import { createSafeStats, safeLargeNumber, safePercentage, safeResponseTime, safeCurrency } from '../../lib/dataValidation'
import { LineChart, Line, CartesianGrid, XAxis, YAxis } from 'recharts'
import {
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from '@/components/ui/chart'

/** 账号 API Key 数据结构 - 与后端保持一致 */
interface LocalProviderKey extends Omit<ProviderKey, 'status'> {
  // 为了兼容现有UI，添加一些别名
  keyName: string  // 映射到 name
  keyValue: string // 映射到 api_key
  status: 'active' | 'disabled', // 基于 is_active 转换，不再用于筛选
  createdAt: string // 映射到 created_at
  requestLimitPerMinute: number // 映射到 max_requests_per_minute
  tokenLimitPromptPerMinute: number // 映射到 max_tokens_prompt_per_minute
  requestLimitPerDay: number // 映射到 max_requests_per_day
  healthStatus: string // 健康状态（用于显示和内部逻辑）
  cost: number // 从 usage.total_cost 映射
  usage: {
    total_requests: number
    successful_requests: number
    failed_requests: number
    success_rate: number
    total_tokens: number
    total_cost: number
    avg_response_time: number
    last_used_at?: string
  } // 使用完整的usage对象结构
  rateLimitRemainingSeconds?: number // 限流剩余时间（秒）
  provider_type_id?: number // 服务商类型ID
  project_id?: string // Gemini OAuth extra project scope
}

interface ProviderKeyFormState {
  provider: string
  provider_type_id: number
  keyName: string
  keyValue: string
  auth_type: string
  weight: number
  requestLimitPerMinute: number
  tokenLimitPromptPerMinute: number
  requestLimitPerDay: number
  status: 'active' | 'disabled'
  project_id?: string
}

type ProviderKeyEditFormState = ProviderKeyFormState & { id: number }

// 健康状态显示文本映射
const getHealthStatusDisplay = (backendStatus: string): { color: string; bg: string; ring: string; text: string } => {
  switch (backendStatus) {
    case 'healthy':
      return { color: 'text-emerald-600', bg: 'bg-emerald-50', ring: 'ring-emerald-200', text: '健康' }
    case 'rate_limited':
      return { color: 'text-yellow-600', bg: 'bg-yellow-50', ring: 'ring-yellow-200', text: '限流中' }
    case 'unhealthy':
      return { color: 'text-red-600', bg: 'bg-red-50', ring: 'ring-red-200', text: '异常' }
    case 'error':
      return { color: 'text-red-600', bg: 'bg-red-50', ring: 'ring-red-200', text: '错误' }
    case 'unknown':
    default:
      return { color: 'text-gray-600', bg: 'bg-gray-50', ring: 'ring-gray-200', text: '未知' }
  }
}


// 限流倒计时钩子
const useRateLimitCountdown = (initialSeconds?: number) => {
  const [remainingSeconds, setRemainingSeconds] = useState<number | null>(initialSeconds || null)

  useEffect(() => {
    if (initialSeconds === undefined || initialSeconds === null) {
      setRemainingSeconds(null)
      return
    }

    setRemainingSeconds(initialSeconds)

    const interval = setInterval(() => {
      setRemainingSeconds(prev => {
        if (prev === null || prev <= 1) {
          clearInterval(interval)
          return null
        }
        return prev - 1
      })
    }, 1000)

    return () => clearInterval(interval)
  }, [initialSeconds])

  const formatTime = useCallback((seconds: number): string => {
    if (seconds < 60) {
      return `${seconds}秒`
    } else if (seconds < 3600) {
      const minutes = Math.floor(seconds / 60)
      const remainingSecs = seconds % 60
      return `${minutes}分${remainingSecs}秒`
    } else {
      const hours = Math.floor(seconds / 3600)
      const minutes = Math.floor((seconds % 3600) / 60)
      return `${hours}小时${minutes}分`
    }
  }, [])

  return {
    remainingSeconds,
    formattedTime: remainingSeconds ? formatTime(remainingSeconds) : null,
    isRateLimited: remainingSeconds !== null && remainingSeconds > 0
  }
}

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
    rateLimitRemainingSeconds: (apiKey.status as any)?.rate_limit_remaining_seconds,
  }
}

const transformProviderKeyToAPI = (localKey: Partial<LocalProviderKey>): any => {
  return {
    name: localKey.keyName,
    api_key: localKey.keyValue,
    is_active: localKey.status === 'active',
    max_requests_per_minute: localKey.requestLimitPerMinute,
    max_tokens_prompt_per_minute: localKey.tokenLimitPromptPerMinute,
    max_requests_per_day: localKey.requestLimitPerDay,
    weight: localKey.weight,
  }
}

/** 弹窗类型 */
type DialogType = 'add' | 'edit' | 'delete' | 'stats' | null

/** 页面主组件 */
const ProviderKeysPage: React.FC = () => {
  // 数据状态
  const [data, setData] = useState<LocalProviderKey[]>([])
  const [dashboardStats, setDashboardStats] = useState<ProviderKeysDashboardStatsResponse | null>(null)
  const [loading, setLoading] = useState(true)
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
    try {
      const response = await api.providerKeys.getDashboardStats()
      if (response.success && response.data) {
        setDashboardStats(response.data)
      } else {
        console.error('获取仪表板统计数据失败:', response.error?.message)
      }
    } catch (error) {
      console.error('获取仪表板统计数据异常:', error)
    }
  }

  // 获取密钥列表数据
  const fetchData = async () => {
    try {
      setLoading(true)
      setError(null)
      
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
      setError(error instanceof Error ? error.message : '获取数据失败')
    } finally {
      setLoading(false)
    }
  }

  // 初始化数据加载
  useEffect(() => {
    fetchDashboardStats()
  }, [])

  useEffect(() => {
    fetchData()
  }, [currentPage, pageSize, searchTerm, statusFilter, providerFilter])

  // 获取所有账号列表
  const providers = useMemo(() => {
    const uniqueProviders = Array.from(new Set(data.map(item => item.provider)))
    return uniqueProviders
  }, [data])

  // 由于后端已经处理了过滤和分页，前端直接使用返回的数据
  const paginatedData = data
  
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
  const handleAdd = async (newKey: Omit<LocalProviderKey, 'id' | 'usage' | 'cost' | 'createdAt' | 'healthCheck'>) => {
    try {
      // 找到对应的provider_type_id
      const providerTypesResponse = await api.auth.getProviderTypes({ is_active: true })
      let providerTypeId = 1 // 默认值
      let matchedType: ProviderType | undefined
      
      if (providerTypesResponse.success && providerTypesResponse.data?.provider_types) {
        matchedType = providerTypesResponse.data.provider_types.find(
          type => type.display_name === newKey.provider
        )
        if (matchedType) {
          providerTypeId = matchedType.id
        }
      }

      const payload: CreateProviderKeyRequest = {
        provider_type_id: providerTypeId,
        name: newKey.keyName,
        api_key: newKey.keyValue || generateApiKey(newKey.provider),
        auth_type: newKey.auth_type || 'api_key',
        weight: newKey.weight || 1,
        max_requests_per_minute: newKey.requestLimitPerMinute || 0,
        max_tokens_prompt_per_minute: newKey.tokenLimitPromptPerMinute || 0,
        max_requests_per_day: newKey.requestLimitPerDay || 0,
        is_active: newKey.status === 'active',
      }

      const projectId = (newKey.project_id || '').trim()
      if (projectId && matchedType?.name === 'gemini' && (newKey.auth_type || '').includes('oauth')) {
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
  const handleEdit = async (updatedKey: LocalProviderKey) => {
    try {
      // 找到对应的provider_type_id
      const providerTypesResponse = await api.auth.getProviderTypes({ is_active: true })
      let providerTypeId = 1 // 默认值
      let matchedType: ProviderType | undefined
      
      if (providerTypesResponse.success && providerTypesResponse.data?.provider_types) {
        matchedType = providerTypesResponse.data.provider_types.find(
          type => type.display_name === updatedKey.provider
        )
        if (matchedType) {
          providerTypeId = matchedType.id
        }
      }

      const payload: UpdateProviderKeyRequest = {
        provider_type_id: providerTypeId,
        name: updatedKey.keyName,
        api_key: updatedKey.keyValue,
        auth_type: updatedKey.auth_type || 'api_key',
        weight: updatedKey.weight,
        max_requests_per_minute: updatedKey.requestLimitPerMinute,
        max_tokens_prompt_per_minute: updatedKey.tokenLimitPromptPerMinute,
        max_requests_per_day: updatedKey.requestLimitPerDay,
        is_active: updatedKey.status === 'active',
      }

      const projectId = (updatedKey.project_id || '').trim()
      if (projectId && matchedType?.name === 'gemini' && (updatedKey.auth_type || '').includes('oauth')) {
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

  // 复制到剪贴板
  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text)
  }

  // 健康状态组件
  const HealthStatus: React.FC<{ healthStatus: string; rateLimitRemainingSeconds?: number }> = ({ healthStatus, rateLimitRemainingSeconds }) => {
    const { remainingSeconds, formattedTime, isRateLimited } = useRateLimitCountdown(rateLimitRemainingSeconds)

    const config = getHealthStatusDisplay(healthStatus)

    // 如果是限流状态且有剩余时间，显示倒计时
    if (healthStatus === 'rate_limited' && isRateLimited && formattedTime) {
      return (
        <div className="flex flex-col items-start gap-1">
          <span className={`inline-flex items-center px-2 py-1 rounded-full text-xs font-medium ${config.bg} ${config.color} ring-1 ${config.ring}`}>
            <Activity size={10} className="mr-1" />
            限流中
          </span>
          <span className="text-xs text-yellow-600 font-medium">
            {formattedTime}后恢复
          </span>
        </div>
      )
    }

    return (
      <span className={`inline-flex items-center px-2 py-1 rounded-full text-xs font-medium ${config.bg} ${config.color} ring-1 ${config.ring}`}>
        <Activity size={10} className="mr-1" />
        {config.text}
      </span>
    )
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
          <h2 className="text-lg font-medium text-neutral-800">账号API Keys</h2>
          <p className="text-sm text-neutral-600 mt-1">管理上游账号的API访问密钥</p>
        </div>
        <div className="flex gap-2">
          <button
            onClick={() => {
              fetchData()
              fetchDashboardStats()
            }}
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
            新增密钥
          </button>
        </div>
      </div>

      {/* 统计信息 */}
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
              {paginatedData.map((item) => (
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
                      请求限制: {item.requestLimitPerMinute}/分钟
                    </div>
                    <div className="text-xs text-neutral-500">
                      Token限制: {item.tokenLimitPromptPerMinute.toLocaleString()}/分钟
                    </div>
                    <div className="text-xs text-neutral-500">
                      请求限制: {item.requestLimitPerDay.toLocaleString()}/天
                    </div>
                  </td>
                  <td className="px-4 py-3">
                    <div className="text-sm font-medium text-neutral-900">${(item.cost || 0).toFixed(2)}</div>
                    <div className="text-xs text-neutral-500">本月花费</div>
                  </td>
                  <td className="px-4 py-3">
                    <div className="flex items-center gap-2">
                      <HealthStatus healthStatus={item.healthStatus} rateLimitRemainingSeconds={item.rateLimitRemainingSeconds} />
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
              ))}
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

/** 对话框门户组件 */
const DialogPortal: React.FC<{
  type: DialogType
  selectedItem: LocalProviderKey | null
  onClose: () => void
  onAdd: (item: Omit<LocalProviderKey, 'id' | 'usage' | 'cost' | 'createdAt' | 'healthCheck'>) => void
  onEdit: (item: LocalProviderKey) => void
  onDelete: () => void
  onRefresh: () => Promise<void>
}> = ({ type, selectedItem, onClose, onAdd, onEdit, onDelete, onRefresh }) => {
  if (!type) return null

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      {type === 'add' && <AddDialog onClose={onClose} onSubmit={onAdd} />}
      {type === 'edit' && selectedItem && <EditDialog item={selectedItem} onClose={onClose} onSubmit={onEdit} />}
      {type === 'delete' && selectedItem && <DeleteDialog item={selectedItem} onClose={onClose} onConfirm={onDelete} />}
      {type === 'stats' && selectedItem && <StatsDialog item={selectedItem} onClose={onClose} />}
    </div>
  )
}

/** 添加对话框 */
const AddDialog: React.FC<{
  onClose: () => void
  onSubmit: (item: Omit<LocalProviderKey, 'id' | 'usage' | 'cost' | 'createdAt' | 'healthCheck'>) => void
}> = ({ onClose, onSubmit }) => {
  const [formData, setFormData] = useState<ProviderKeyFormState>({
    provider: '',
    provider_type_id: 0,
    keyName: '',
    keyValue: '',
    auth_type: 'api_key',
    weight: 1,
    requestLimitPerMinute: 0,
    tokenLimitPromptPerMinute: 0,
    requestLimitPerDay: 0,
    status: 'active', // 保持为启用状态，不影响健康状态筛选
    project_id: '', // 新增 Gemini 项目ID字段
  })

  // 服务商类型状态管理
  const [providerTypes, setProviderTypes] = useState<ProviderType[]>([])
  const [loadingProviderTypes, setLoadingProviderTypes] = useState(true)
  const [selectedProviderType, setSelectedProviderType] = useState<ProviderType | null>(null)
  
  // OAuth相关状态
  const [oauthStatus, setOAuthStatus] = useState<OAuthStatus>('idle')
  const [oauthExtraParams, setOAuthExtraParams] = useState<{ [key: string]: string }>({})

  // 获取服务商类型列表
  const fetchProviderTypes = async () => {
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
            provider: firstProvider.display_name,
            provider_type_id: firstProvider.id
          }))
          setSelectedProviderType(firstProvider)
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

  // 初始化：获取服务商类型
  React.useEffect(() => {
    fetchProviderTypes()
  }, [])

  // OAuth处理函数
  const handleOAuthComplete = async (result: OAuthResult) => {
    console.log('=== OAuth完成回调开始 ===')
    console.log('OAuth完成结果:', result)
    console.log('当前formData状态:', formData)
    
    if (result.success && result.data) {
      console.log('OAuth数据详情:', {
        access_token: result.data.access_token,
        refresh_token: result.data.refresh_token,
        token_type: result.data.token_type,
        expires_in: result.data.expires_in,
        auth_status: result.data.auth_status,
        完整data对象: result.data
      })
      
      // OAuth成功完成，将获取到的token填充到表单
      setOAuthStatus('success')
      
      // 将OAuth返回的session_id填入表单的API密钥字段 (OAuth类型需要session_id而不是access_token)
      const newKeyValue = result.data.session_id
      console.log('准备填充的session_id:', newKeyValue)
      console.log('session_id类型:', typeof newKeyValue)
      console.log('session_id长度:', newKeyValue ? newKeyValue.length : 0)
      console.log('session_id是否为空:', !newKeyValue)
      
      setFormData(prev => {
        const newFormData = {
          ...prev,
          keyValue: newKeyValue,
        }
        console.log('更新前的formData:', prev)
        console.log('更新后的formData:', newFormData)
        console.log('keyValue变更:', prev.keyValue, '=>', newFormData.keyValue)
        return newFormData
      })
      
      // 延迟检查状态更新是否生效  
      setTimeout(() => {
        console.log('延迟检查 - 当前formData.keyValue:', formData.keyValue)
        const inputElement = document.querySelector('input[placeholder*="API密钥"]') as HTMLInputElement
        console.log('延迟检查 - 输入框实际值:', inputElement?.value)
      }, 100)
      
      // 显示成功消息，提示用户可以看到token并决定是否提交
      toast.success('OAuth授权成功！', {
        description: 'Token已填充到API密钥字段，请检查并完善其他信息后点击"添加"按钮提交。',
        duration: 5000,
      })
    } else {
      setOAuthStatus('error')
      toast.error('OAuth授权失败', {
        description: result.error || 'OAuth授权过程中发生错误，请重试',
        duration: 5000,
      })
      console.error('OAuth失败:', result.error)
    }
    console.log('=== OAuth完成回调结束 ===')
  }

  const handleProviderTypeChange = (value: string) => {
    const selectedProvider = providerTypes.find(type => type.display_name === value)
    if (selectedProvider) {
      setFormData(prev => ({ 
        ...prev, 
        provider: selectedProvider.display_name,
        provider_type_id: selectedProvider.id
      }))
      setSelectedProviderType(selectedProvider)
      // 重置OAuth状态和认证类型
      setOAuthStatus('idle')
      setFormData(prev => ({ ...prev, auth_type: 'api_key' }))
      setOAuthExtraParams({})
    }
  }

  const handleAuthTypeChange = (authType: string) => {
    setFormData(prev => ({ ...prev, auth_type: authType }))
    // 如果切换到非OAuth类型，重置OAuth状态
    if (!authType.includes('oauth')) {
      setOAuthStatus('idle')
    }
    // 重置额外参数
    setOAuthExtraParams({})
  }

  // 获取当前认证类型的额外参数配置
  const getCurrentAuthExtraParams = (): Array<{
    key: string
    label: string
    default: string
    required: boolean
    type: string
    placeholder?: string
    description?: string
  }> => {
    if (!selectedProviderType?.auth_configs) return []
    
    const authConfigs = selectedProviderType.auth_configs as any
    const currentAuthConfig = authConfigs[formData.auth_type]
    
    return currentAuthConfig?.extra_params || []
  }

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    // OAuth类型的密钥需要先完成OAuth流程
    if (formData.auth_type.includes('oauth') && oauthStatus !== 'success') {
      toast.info('请先完成OAuth授权流程')
      return
    }
    onSubmit({
      name: formData.keyName,
      api_key: formData.keyValue,
      provider: formData.provider,
      auth_type: formData.auth_type,
      weight: formData.weight,
      max_requests_per_minute: formData.requestLimitPerMinute,
      max_tokens_prompt_per_minute: formData.tokenLimitPromptPerMinute,
      max_requests_per_day: formData.requestLimitPerDay,
      is_active: formData.status === 'active',
      keyName: formData.keyName,
      keyValue: formData.keyValue,
      requestLimitPerMinute: formData.requestLimitPerMinute,
      tokenLimitPromptPerMinute: formData.tokenLimitPromptPerMinute,
      requestLimitPerDay: formData.requestLimitPerDay,
      status: formData.status,
      provider_type_id: formData.provider_type_id,
      project_id: formData.project_id,
    } as any)
  }

  // 处理数字输入框的增减
  const handleNumberChange = (field: string, delta: number) => {
    setFormData(prev => ({
      ...prev,
      [field]: Math.max(0, (prev[field as keyof typeof prev] as number) + delta)
    }))
  }

  return (
    <div className="bg-white rounded-2xl p-6 w-full max-w-lg mx-4 max-h-[80vh] overflow-y-auto border border-neutral-200 hover:shadow-sm transition-shadow">
      <h3 className="text-lg font-medium text-neutral-900 mb-4">新增账号密钥</h3>
      <form onSubmit={handleSubmit} className="space-y-4">
        {/* 密钥名称 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">
            <span className="text-red-500">*</span> 密钥名称
          </label>
          <input
            type="text"
            required
            value={formData.keyName}
            onChange={(e) => setFormData({ ...formData, keyName: e.target.value })}
            placeholder="请输入密钥名称"
            className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>

        {/* 服务商类型 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">
            <span className="text-red-500">*</span> 服务商类型
          </label>
          
          {loadingProviderTypes ? (
            <div className="flex items-center gap-2 p-3 border border-neutral-200 rounded-lg">
              <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-violet-600"></div>
              <span className="text-sm text-neutral-600">加载服务商类型...</span>
            </div>
          ) : (
            <ModernSelect
              value={formData.provider}
              onValueChange={handleProviderTypeChange}
              options={providerTypes.map(type => ({
                value: type.display_name,
                label: type.display_name
              }))}
              placeholder="请选择服务商类型"
            />
          )}
        </div>

        {/* 认证类型 */}
        {selectedProviderType && (
          <div>
            <label className="block text-sm font-medium text-neutral-700 mb-1">
              <span className="text-red-500">*</span> 认证类型
            </label>
            <AuthTypeSelector
              providerType={selectedProviderType}
              value={formData.auth_type}
              onValueChange={handleAuthTypeChange}
            />
          </div>
        )}

        {/* 动态额外参数字段 */}
        {selectedProviderType && formData.auth_type.includes('oauth') && getCurrentAuthExtraParams().length > 0 && (
          <div>
            <label className="block text-sm font-medium text-neutral-700 mb-2">OAuth额外参数</label>
            {getCurrentAuthExtraParams().map((param) => (
              <div key={param.key} className="mb-3">
                <label className="block text-sm font-medium text-neutral-700 mb-1">
                  {param.required && <span className="text-red-500">*</span>} {param.label}
                </label>
                <input
                  type={param.type === 'number' ? 'number' : 'text'}
                  required={param.required}
                  value={oauthExtraParams[param.key] || param.default || ''}
                  onChange={(e) => setOAuthExtraParams(prev => ({
                    ...prev,
                    [param.key]: e.target.value
                  }))}
                  placeholder={param.placeholder}
                  className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
                />
                {param.description && (
                  <p className="text-xs text-neutral-600 mt-1">{param.description}</p>
                )}
              </div>
            ))}
          </div>
        )}

        {/* OAuth Handler */}
        {selectedProviderType && formData.auth_type.includes('oauth') && (
          <div>
            <label className="block text-sm font-medium text-neutral-700 mb-1">
              OAuth授权
            </label>
            <OAuthHandler
              request={{
                provider_name: selectedProviderType.name,
                name: formData.keyName || 'Provider Key',
                description: `${selectedProviderType.display_name} OAuth Key`,
                extra_params: oauthExtraParams,
              }}
              status={oauthStatus}
              onStatusChange={setOAuthStatus}
              onComplete={handleOAuthComplete}
            />
          </div>
        )}

        {/* API密钥 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">
            <span className="text-red-500">*</span> API密钥
          </label>
          <input
            type="text"
            required={!formData.auth_type.includes('oauth')}
            value={formData.keyValue}
            onChange={(e) => setFormData({ ...formData, keyValue: e.target.value })}
            placeholder={
              formData.auth_type.includes('oauth') 
                ? "OAuth授权完成后自动填入" 
                : "请输入API密钥"
            }
            disabled={formData.auth_type.includes('oauth')}
            className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40 disabled:bg-neutral-50 disabled:text-neutral-500"
          />
        </div>

        {/* Gemini 项目ID - 仅在选择 Gemini 时显示 */}
        {selectedProviderType && selectedProviderType.name === 'gemini' && (
          <div>
            <label className="block text-sm font-medium text-neutral-700 mb-1">
              项目ID
              <span className="text-xs text-neutral-500 ml-2">
                （可选，用于 Google Cloud Code Assist）
              </span>
            </label>
            <input
              type="text"
              value={formData.project_id || ''}
              onChange={(e) => setFormData({ ...formData, project_id: e.target.value })}
              placeholder="请输入 Google Cloud 项目ID"
              className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
            />
            <p className="text-xs text-neutral-600 mt-1">
              留空使用标准 Gemini API，填写则使用 Code Assist API
            </p>
          </div>
        )}

        {/* 权重 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">权重</label>
          <div className="flex items-center">
            <button
              type="button"
              onClick={() => handleNumberChange('weight', -1)}
              className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
            >
              −
            </button>
            <input
              type="number"
              min="0"
              value={formData.weight}
              onChange={(e) => setFormData({ ...formData, weight: parseInt(e.target.value) || 0 })}
              className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
            />
            <button
              type="button"
              onClick={() => handleNumberChange('weight', 1)}
              className="px-3 py-2 border border-neutral-200 rounded-r-lg text-neutral-600 hover:bg-neutral-50"
            >
              +
            </button>
          </div>
        </div>

        {/* 请求限制/分钟 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">请求限制/分钟</label>
          <div className="flex items-center">
            <button
              type="button"
              onClick={() => handleNumberChange('requestLimitPerMinute', -1)}
              className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
            >
              −
            </button>
            <input
              type="number"
              min="0"
              value={formData.requestLimitPerMinute}
              onChange={(e) => setFormData({ ...formData, requestLimitPerMinute: parseInt(e.target.value) || 0 })}
              className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
            />
            <button
              type="button"
              onClick={() => handleNumberChange('requestLimitPerMinute', 1)}
              className="px-3 py-2 border border-neutral-200 rounded-r-lg text-neutral-600 hover:bg-neutral-50"
            >
              +
            </button>
          </div>
        </div>

        {/* Token限制/分钟 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">Token限制/分钟</label>
          <div className="flex items-center">
            <button
              type="button"
              onClick={() => handleNumberChange('tokenLimitPromptPerMinute', -10)}
              className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
            >
              −
            </button>
            <input
              type="number"
              min="0"
              value={formData.tokenLimitPromptPerMinute}
              onChange={(e) => setFormData({ ...formData, tokenLimitPromptPerMinute: parseInt(e.target.value) || 0 })}
              className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
            />
            <button
              type="button"
              onClick={() => handleNumberChange('tokenLimitPromptPerMinute', 10)}
              className="px-3 py-2 border border-neutral-200 rounded-r-lg text-neutral-600 hover:bg-neutral-50"
            >
              +
            </button>
          </div>
        </div>

        {/* 请求限制/天 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">请求限制/天</label>
          <div className="flex items-center">
            <button
              type="button"
              onClick={() => handleNumberChange('requestLimitPerDay', -100)}
              className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
            >
              −
            </button>
            <input
              type="number"
              min="0"
              value={formData.requestLimitPerDay}
              onChange={(e) => setFormData({ ...formData, requestLimitPerDay: parseInt(e.target.value) || 0 })}
              className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
            />
            <button
              type="button"
              onClick={() => handleNumberChange('requestLimitPerDay', 100)}
              className="px-3 py-2 border border-neutral-200 rounded-r-lg text-neutral-600 hover:bg-neutral-50"
            >
              +
            </button>
          </div>
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
  item: LocalProviderKey
  onClose: () => void
  onSubmit: (item: LocalProviderKey) => void
}> = ({ item, onClose, onSubmit }) => {
  const [formData, setFormData] = useState<ProviderKeyEditFormState>({
    id: Number(item.id),
    provider: item.provider,
    provider_type_id: item.provider_type_id || 0,
    keyName: item.keyName,
    keyValue: item.keyValue,
    auth_type: item.auth_type || 'api_key',
    weight: item.weight,
    requestLimitPerMinute: item.requestLimitPerMinute,
    tokenLimitPromptPerMinute: item.tokenLimitPromptPerMinute,
    requestLimitPerDay: item.requestLimitPerDay,
    status: item.status,
    project_id: item.project_id || ''
  })

  // 服务商类型状态管理
  const [providerTypes, setProviderTypes] = useState<ProviderType[]>([])
  const [loadingProviderTypes, setLoadingProviderTypes] = useState(true)
  const [selectedProviderType, setSelectedProviderType] = useState<ProviderType | null>(null)
  
  // OAuth相关状态
  const [oauthStatus, setOAuthStatus] = useState<OAuthStatus>('idle')
  const [oauthExtraParams, setOAuthExtraParams] = useState<{ [key: string]: string }>({})

  // 获取服务商类型列表
  const fetchProviderTypes = async () => {
    setLoadingProviderTypes(true)
    try {
      const response = await api.auth.getProviderTypes({ is_active: true })
      
      if (response.success && response.data) {
        setProviderTypes(response.data.provider_types || [])
        // 根据当前item的provider设置selectedProviderType
        const currentProvider = response.data.provider_types?.find(
          type => type.display_name === item.provider
        )
        if (currentProvider) {
          setSelectedProviderType(currentProvider)
        }
      } else {
        console.error('[EditDialog] 获取服务商类型失败:', response.message)
      }
    } catch (err) {
      console.error('[EditDialog] 获取服务商类型异常:', err)
    } finally {
      setLoadingProviderTypes(false)
    }
  }

  // 初始化：获取服务商类型
  React.useEffect(() => {
    fetchProviderTypes()
  }, [])

  // OAuth处理函数
  const handleOAuthComplete = async (result: OAuthResult) => {
    if (result.success && result.data) {
      // OAuth成功完成，将获取到的token填充到表单
      setOAuthStatus('success')
      
      // 将OAuth返回的session_id填入表单的API密钥字段 (OAuth类型需要session_id而不是access_token)
      const newKeyValue = result.data.session_id
      
      setFormData(prev => ({
        ...prev,
        keyValue: newKeyValue,
      }))
      
      // 显示成功消息，提示用户可以看到token并决定是否提交
      toast.success('OAuth授权成功！', {
        description: 'OAuth会话ID已填充到API密钥字段，请检查并完善其他信息后点击"保存修改"按钮提交。',
        duration: 5000,
      })
    } else {
      setOAuthStatus('error')
      toast.error('OAuth授权失败', {
        description: result.error || 'OAuth授权过程中发生错误，请重试',
        duration: 5000,
      })
    }
  }

  const handleProviderTypeChange = (value: string) => {
    const selectedProvider = providerTypes.find(type => type.display_name === value)
    if (selectedProvider) {
      setFormData(prev => ({ 
        ...prev, 
        provider: selectedProvider.display_name,
        provider_type_id: selectedProvider.id
      }))
      setSelectedProviderType(selectedProvider)
      // 重置OAuth状态和认证类型
      setOAuthStatus('idle')
      setFormData(prev => ({ ...prev, auth_type: 'api_key' }))
      setOAuthExtraParams({})
    }
  }

  const handleAuthTypeChange = (authType: string) => {
    setFormData(prev => ({ ...prev, auth_type: authType }))
    // 如果切换到非OAuth类型，重置OAuth状态
    if (!authType.includes('oauth')) {
      setOAuthStatus('idle')
    }
    // 重置额外参数
    setOAuthExtraParams({})
  }

  // 获取当前认证类型的额外参数配置
  const getCurrentAuthExtraParams = (): Array<{
    key: string
    label: string
    default: string
    required: boolean
    type: string
    placeholder?: string
    description?: string
  }> => {
    if (!selectedProviderType?.auth_configs) return []
    
    const authConfigs = selectedProviderType.auth_configs as any
    const currentAuthConfig = authConfigs[formData.auth_type]
    
    return currentAuthConfig?.extra_params || []
  }

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    // OAuth类型的密钥需要先完成OAuth流程
    if (formData.auth_type.includes('oauth') && oauthStatus !== 'success') {
      toast.info('请先完成OAuth授权流程')
      return
    }
    onSubmit({
      id: formData.id,
      name: formData.keyName,
      api_key: formData.keyValue,
      provider: formData.provider,
      auth_type: formData.auth_type,
      weight: formData.weight,
      max_requests_per_minute: formData.requestLimitPerMinute,
      max_tokens_prompt_per_minute: formData.tokenLimitPromptPerMinute,
      max_requests_per_day: formData.requestLimitPerDay,
      is_active: formData.status === 'active',
      keyName: formData.keyName,
      keyValue: formData.keyValue,
      requestLimitPerMinute: formData.requestLimitPerMinute,
      tokenLimitPromptPerMinute: formData.tokenLimitPromptPerMinute,
      requestLimitPerDay: formData.requestLimitPerDay,
      status: formData.status,
      provider_type_id: formData.provider_type_id,
      project_id: formData.project_id,
    } as any)
  }

  // 处理数字输入框的增减
  const handleNumberChange = (field: string, delta: number) => {
    setFormData(prev => ({
      ...prev,
      [field]: Math.max(0, (prev[field as keyof typeof prev] as number) + delta)
    }))
  }

  return (
    <div className="bg-white rounded-2xl p-6 w-full max-w-lg mx-4 max-h-[80vh] overflow-y-auto border border-neutral-200 hover:shadow-sm transition-shadow">
      <h3 className="text-lg font-medium text-neutral-900 mb-4">编辑账号密钥</h3>
      <form onSubmit={handleSubmit} className="space-y-4">
        {/* 密钥名称 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">
            <span className="text-red-500">*</span> 密钥名称
          </label>
          <input
            type="text"
            required
            value={formData.keyName}
            onChange={(e) => setFormData({ ...formData, keyName: e.target.value })}
            className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>

        {/* 服务商类型 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">
            <span className="text-red-500">*</span> 服务商类型
          </label>
          
          {loadingProviderTypes ? (
            <div className="flex items-center gap-2 p-3 border border-neutral-200 rounded-lg">
              <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-violet-600"></div>
              <span className="text-sm text-neutral-600">加载服务商类型...</span>
            </div>
          ) : (
            <ModernSelect
              value={formData.provider}
              onValueChange={handleProviderTypeChange}
              options={providerTypes.map(type => ({
                value: type.display_name,
                label: type.display_name
              }))}
              placeholder="请选择服务商类型"
            />
          )}
        </div>

        {/* 认证类型 */}
        {selectedProviderType && (
          <div>
            <label className="block text-sm font-medium text-neutral-700 mb-1">
              <span className="text-red-500">*</span> 认证类型
            </label>
            <AuthTypeSelector
              providerType={selectedProviderType}
              value={formData.auth_type || 'api_key'}
              onValueChange={handleAuthTypeChange}
            />
          </div>
        )}

        {/* 动态额外参数字段 */}
        {selectedProviderType && formData.auth_type.includes('oauth') && getCurrentAuthExtraParams().length > 0 && (
          <div>
            <label className="block text-sm font-medium text-neutral-700 mb-2">OAuth额外参数</label>
            {getCurrentAuthExtraParams().map((param) => (
              <div key={param.key} className="mb-3">
                <label className="block text-sm font-medium text-neutral-700 mb-1">
                  {param.required && <span className="text-red-500">*</span>} {param.label}
                </label>
                <input
                  type={param.type === 'number' ? 'number' : 'text'}
                  required={param.required}
                  value={oauthExtraParams[param.key] || param.default || ''}
                  onChange={(e) => setOAuthExtraParams(prev => ({
                    ...prev,
                    [param.key]: e.target.value
                  }))}
                  placeholder={param.placeholder}
                  className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
                />
                {param.description && (
                  <p className="text-xs text-neutral-600 mt-1">{param.description}</p>
                )}
              </div>
            ))}
          </div>
        )}

        {/* OAuth Handler */}
        {selectedProviderType && formData.auth_type.includes('oauth') && (
          <div>
            <label className="block text-sm font-medium text-neutral-700 mb-1">
              OAuth授权
            </label>
            <OAuthHandler
              request={{
                provider_name: selectedProviderType.name,
                name: formData.keyName || 'Provider Key',
                description: `${selectedProviderType.display_name} OAuth Key`,
                extra_params: oauthExtraParams,
              }}
              status={oauthStatus}
              onStatusChange={setOAuthStatus}
              onComplete={handleOAuthComplete}
            />
          </div>
        )}

        {/* API密钥 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">
            <span className="text-red-500">*</span> API密钥
          </label>
          <input
            type="text"
            required={!formData.auth_type.includes('oauth')}
            value={formData.keyValue}
            onChange={(e) => setFormData({ ...formData, keyValue: e.target.value })}
            placeholder={
              formData.auth_type.includes('oauth') 
                ? "OAuth授权完成后自动填入" 
                : "请输入API密钥"
            }
            disabled={formData.auth_type.includes('oauth')}
            className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40 disabled:bg-neutral-50 disabled:text-neutral-500"
          />
        </div>

        {/* Gemini 项目ID - 仅在选择 Gemini 时显示 */}
        {selectedProviderType && selectedProviderType.name === 'gemini' && (
          <div>
            <label className="block text-sm font-medium text-neutral-700 mb-1">
              项目ID
              <span className="text-xs text-neutral-500 ml-2">
                （可选，用于 Google Cloud Code Assist）
              </span>
            </label>
            <input
              type="text"
              value={formData.project_id || ''}
              onChange={(e) => setFormData({ ...formData, project_id: e.target.value })}
              placeholder="请输入 Google Cloud 项目ID"
              className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
            />
            <p className="text-xs text-neutral-600 mt-1">
              留空使用标准 Gemini API，填写则使用 Code Assist API
            </p>
          </div>
        )}

        {/* 权重 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">权重</label>
          <div className="flex items-center">
            <button
              type="button"
              onClick={() => handleNumberChange('weight', -1)}
              className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
            >
              −
            </button>
            <input
              type="number"
              min="0"
              value={formData.weight}
              onChange={(e) => setFormData({ ...formData, weight: parseInt(e.target.value) || 0 })}
              className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
            />
            <button
              type="button"
              onClick={() => handleNumberChange('weight', 1)}
              className="px-3 py-2 border border-neutral-200 rounded-r-lg text-neutral-600 hover:bg-neutral-50"
            >
              +
            </button>
          </div>
        </div>

        {/* 请求限制/分钟 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">请求限制/分钟</label>
          <div className="flex items-center">
            <button
              type="button"
              onClick={() => handleNumberChange('requestLimitPerMinute', -1)}
              className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
            >
              −
            </button>
            <input
              type="number"
              min="0"
              value={formData.requestLimitPerMinute}
              onChange={(e) => setFormData({ ...formData, requestLimitPerMinute: parseInt(e.target.value) || 0 })}
              className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
            />
            <button
              type="button"
              onClick={() => handleNumberChange('requestLimitPerMinute', 1)}
              className="px-3 py-2 border border-neutral-200 rounded-r-lg text-neutral-600 hover:bg-neutral-50"
            >
              +
            </button>
          </div>
        </div>

        {/* Token限制/分钟 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">Token限制/分钟</label>
          <div className="flex items-center">
            <button
              type="button"
              onClick={() => handleNumberChange('tokenLimitPromptPerMinute', -10)}
              className="px-3 py-2 border border-neutral-200 rounded-l-lg text-neutral-600 hover:bg-neutral-50"
            >
              −
            </button>
            <input
              type="number"
              min="0"
              value={formData.tokenLimitPromptPerMinute}
              onChange={(e) => setFormData({ ...formData, tokenLimitPromptPerMinute: parseInt(e.target.value) || 0 })}
              className="w-full px-3 py-2 border-t border-b border-neutral-200 text-center text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
            />
            <button
              type="button"
              onClick={() => handleNumberChange('tokenLimitPromptPerMinute', 10)}
              className="px-3 py-2 border border-neutral-200 rounded-r-lg text-neutral-600 hover:bg-neutral-50"
            >
              +
            </button>
          </div>
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
  item: LocalProviderKey
  onClose: () => void
  onConfirm: () => void
}> = ({ item, onClose, onConfirm }) => {
  return (
    <div className="bg-white rounded-2xl p-6 w-full max-w-md mx-4 border border-neutral-200 hover:shadow-sm transition-shadow">
      <h3 className="text-lg font-medium text-neutral-900 mb-2">确认删除</h3>
      <p className="text-sm text-neutral-600 mb-4">
        确定要删除 <strong>{item.provider}</strong> 的密钥 <strong>{item.keyName}</strong> 吗？此操作无法撤销。
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
interface ProviderKeyTrendPoint {
  date: string
  requests: number
  cost: number
}

const StatsDialog: React.FC<{
  item: LocalProviderKey
  onClose: () => void
}> = ({ item, onClose }) => {
  // 使用数据验证工具创建安全的统计数据
  const usageStats = createSafeStats(item.usage)

  // 趋势数据状态管理
  const [trendSeries, setTrendSeries] = useState<ProviderKeyTrendPoint[]>([])
  const [trendLoading, setTrendLoading] = useState(true)

  // 获取趋势数据
  useEffect(() => {
    const fetchTrendData = async () => {
      try {
        setTrendLoading(true)
        const response = await api.providerKeys.getTrends(item.id.toString(), { days: 7 })
        if (response.success && response.data && Array.isArray(response.data.trend_data)) {
          const formatted = response.data.trend_data.map((point: any) => ({
            date: typeof point?.date === 'string' ? point.date : '',
            requests: Number(point?.requests ?? 0),
            cost: Number(point?.cost ?? 0),
          })) as ProviderKeyTrendPoint[]

          const withSortedDates = formatted.some((p) => p.date)
            ? [...formatted].sort((a, b) => {
                const aTime = new Date(a.date).getTime()
                const bTime = new Date(b.date).getTime()
                if (Number.isNaN(aTime) || Number.isNaN(bTime)) {
                  return 0
                }
                return aTime - bTime
              })
            : formatted

          setTrendSeries(withSortedDates)
        } else {
          setTrendSeries([])
        }
      } catch (error) {
        console.error('获取趋势数据失败:', error)
        setTrendSeries([])
      } finally {
        setTrendLoading(false)
      }
    }

    fetchTrendData()
  }, [item.id])

  const stats = {
    ...usageStats,
  }

  const chartSeries = trendSeries

  const formatDateLabel = (value: string) => {
    const parsed = new Date(value)
    if (Number.isNaN(parsed.getTime())) {
      return value
    }
    return `${parsed.getMonth() + 1}/${parsed.getDate()}`
  }

  const trendChartData = useMemo(
    () =>
      chartSeries.map((point, index) => ({
        ...point,
        label: formatDateLabel(point.date) || `Day ${index + 1}`,
      })),
    [chartSeries]
  )

  const successRateDisplay = useMemo(
    () => safePercentage(stats.successRate).toFixed(2),
    [stats.successRate]
  )

  const chartConfig = {
    requests: {
      label: "请求数",
      color: "hsl(var(--chart-1))",
    },
    cost: {
      label: "花费",
      color: "hsl(var(--chart-2))",
    },
  } satisfies ChartConfig

  return (
    <div className="bg-white rounded-2xl p-6 w-full max-w-3xl mx-4 max-h-[80vh] overflow-y-auto border border-neutral-200 hover:shadow-sm transition-shadow">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-lg font-medium text-neutral-900">账号密钥统计</h3>
        <button
          onClick={onClose}
          className="text-neutral-500 hover:text-neutral-700"
        >
          ×
        </button>
      </div>
      
      <div className="space-y-6">
        {/* 基本信息 */}
        <div className="grid grid-cols-3 gap-4">
          <div className="p-4 bg-neutral-50 rounded-xl">
            <div className="text-sm text-neutral-600">账号</div>
            <div className="font-medium">{item.provider}</div>
          </div>
          <div className="p-4 bg-neutral-50 rounded-xl">
            <div className="text-sm text-neutral-600">密钥名称</div>
            <div className="font-medium">{item.keyName}</div>
          </div>
          <div className="p-4 bg-neutral-50 rounded-xl">
            <div className="text-sm text-neutral-600">权重</div>
            <div className="font-medium">权重 {item.weight}</div>
          </div>
        </div>

        {/* 使用统计 */}
        <div className="grid grid-cols-4 gap-4">
          <div className="p-4 bg-violet-50 rounded-xl">
            <div className="text-sm text-violet-600">使用次数</div>
            <div className="text-2xl font-bold text-violet-900">{safeLargeNumber(stats.totalRequests)}</div>
          </div>
          <div className="p-4 bg-orange-50 rounded-xl">
            <div className="text-sm text-orange-600">总花费</div>
            <div className="text-2xl font-bold text-orange-900">{safeCurrency(stats.totalCost)}</div>
          </div>
          <div className="p-4 bg-emerald-50 rounded-xl">
            <div className="text-sm text-emerald-600">成功率</div>
            <div className="text-2xl font-bold text-emerald-900">{successRateDisplay}%</div>
          </div>
          <div className="p-4 bg-blue-50 rounded-xl">
            <div className="text-sm text-blue-600">平均响应时间</div>
            <div className="text-2xl font-bold text-blue-900">{safeResponseTime(stats.avgResponseTime)}</div>
          </div>
        </div>

        {/* 使用与花费趋势 */}
        <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
          <div>
            <h4 className="text-sm font-medium text-neutral-900 mb-3">7天使用趋势</h4>
            <div className="h-40 w-full">
              {trendLoading ? (
                <div className="flex h-full items-center justify-center text-neutral-500">
                  <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-violet-600"></div>
                </div>
              ) : chartSeries.length === 0 ? (
                <div className="flex h-full items-center justify-center text-neutral-400 text-sm">
                  暂无趋势数据
                </div>
              ) : (
                <ChartContainer config={chartConfig} className="w-full h-full">
                  <LineChart data={trendChartData}>
                    <CartesianGrid vertical={false} />
                    <XAxis
                      dataKey="label"
                      tickLine={false}
                      axisLine={false}
                      tickMargin={8}
                    />
                    <YAxis
                      tickLine={false}
                      axisLine={false}
                      tickMargin={8}
                    />
                    <ChartTooltip
                      cursor={false}
                      content={<ChartTooltipContent indicator="dot" />}
                    />
                    <Line
                      type="monotone"
                      dataKey="requests"
                      stroke="var(--color-requests)"
                      strokeWidth={2}
                      dot={{ r: 3, strokeWidth: 2 }}
                      activeDot={{ r: 5 }}
                    />
                  </LineChart>
                </ChartContainer>
              )}
            </div>
          </div>

          <div>
            <h4 className="text-sm font-medium text-neutral-900 mb-3">7天花费趋势</h4>
            <div className="h-40 w-full">
              {trendLoading ? (
                <div className="flex h-full items-center justify-center text-neutral-500">
                  <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-violet-600"></div>
                </div>
              ) : chartSeries.length === 0 ? (
                <div className="flex h-full items-center justify-center text-neutral-400 text-sm">
                  暂无趋势数据
                </div>
              ) : (
                <ChartContainer config={chartConfig} className="w-full h-full">
                  <LineChart data={trendChartData}>
                    <CartesianGrid vertical={false} />
                    <XAxis
                      dataKey="label"
                      tickLine={false}
                      axisLine={false}
                      tickMargin={8}
                    />
                    <YAxis
                      tickFormatter={(value: number) => `$${Number(value).toFixed(2)}`}
                      tickLine={false}
                      axisLine={false}
                      tickMargin={8}
                    />
                    <ChartTooltip
                      cursor={false}
                      content={<ChartTooltipContent indicator="dot" />}
                    />
                    <Line
                      type="monotone"
                      dataKey="cost"
                      stroke="var(--color-cost)"
                      strokeWidth={2}
                      dot={{ r: 3, strokeWidth: 2 }}
                      activeDot={{ r: 5 }}
                    />
                  </LineChart>
                </ChartContainer>
              )}
            </div>
          </div>
        </div>

        {/* 详细统计 */}
        <div className="grid grid-cols-3 gap-4">
          <div className="p-4 bg-neutral-50 rounded-xl">
            <div className="text-sm text-neutral-600">总Token数</div>
            <div className="text-2xl font-bold text-neutral-900">
              {safeLargeNumber(stats.totalTokens)}
            </div>
          </div>
          <div className="p-4 bg-neutral-50 rounded-xl">
            <div className="text-sm text-neutral-600">成功请求数</div>
            <div className="text-2xl font-bold text-emerald-900">
              {safeLargeNumber(stats.successfulRequests)}
            </div>
          </div>
          <div className="p-4 bg-neutral-50 rounded-xl">
            <div className="text-sm text-neutral-600">失败请求数</div>
            <div className="text-2xl font-bold text-red-900">
              {safeLargeNumber(stats.failedRequests)}
            </div>
          </div>
        </div>

        {/* 限制配置 */}
        <div>
          <h4 className="text-sm font-medium text-neutral-900 mb-3">限制配置</h4>
          <div className="grid grid-cols-2 gap-4">
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">请求限制</div>
              <div className="font-medium">{item.requestLimitPerMinute} 次/分钟</div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">Token限制</div>
              <div className="font-medium">{(item.tokenLimitPromptPerMinute || 0).toLocaleString()} Token/分钟</div>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}

export default ProviderKeysPage

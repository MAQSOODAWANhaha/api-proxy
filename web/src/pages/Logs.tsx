/**
 * Logs.tsx
 * 请求记录页面：完整的请求记录数据展示、搜索过滤和分页功能
 */

import React, { useState, useEffect, useCallback } from 'react'
import {
  Search,
  RefreshCw,
  Calendar,
  XCircle,
  CheckCircle,
  ChevronLeft,
  ChevronRight,
  Eye,
  FileText,
  Timer,
} from 'lucide-react'
import { StatCard } from '../components/common/StatCard'
import FilterSelect from '../components/common/FilterSelect'
import ModernSelect from '../components/common/ModernSelect'
import { api, ProxyTraceEntry, ProxyTraceListEntry, LogsDashboardStatsResponse } from '../lib/api'
import { LoadingSpinner, LoadingState } from '@/components/ui/loading'

// 使用从 api.ts 导出的日志数据接口

/** 弹窗类型 */
type DialogType = 'details' | null

/** 页面主组件 */
const parseJsonString = (value: string): unknown | null => {
  const trimmed = value.trim()
  if (!trimmed || (trimmed[0] !== '{' && trimmed[0] !== '[')) {
    return null
  }

  try {
    return JSON.parse(trimmed)
  } catch (error) {
    console.warn('解析 JSON 字符串失败:', error)
    return null
  }
}

const normalizeErrorPayload = (value: unknown): unknown => {
  if (Array.isArray(value)) {
    return value.map(normalizeErrorPayload)
  }

  if (value && typeof value === 'object') {
    const entries = Object.entries(value as Record<string, unknown>).map(([key, val]) => {
      if (typeof val === 'string') {
        const nested = parseJsonString(val)
        return [key, nested ? normalizeErrorPayload(nested) : val]
      }

      return [key, normalizeErrorPayload(val)]
    })

    return Object.fromEntries(entries)
  }

  return value
}

const tryParseErrorPayload = (value?: string) => {
  if (!value) return null
  const parsed = parseJsonString(value)
  if (!parsed) return null
  return normalizeErrorPayload(parsed)
}

const LogsPage: React.FC = () => {
  // 数据状态
  const [data, setData] = useState<ProxyTraceListEntry[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [dashboardStats, setDashboardStats] = useState<LogsDashboardStatsResponse | null>(null)
  
  // UI状态
  const [searchTerm, setSearchTerm] = useState('')
  const [methodFilter, setMethodFilter] = useState<string>('all')
  const [statusFilter, setStatusFilter] = useState<string>('all')
  const [userKeyNameFilter, setUserKeyNameFilter] = useState('')
  const [providerKeyNameFilter, setProviderKeyNameFilter] = useState('')
  const [selectedItem, setSelectedItem] = useState<ProxyTraceEntry | null>(null)
  const [dialogType, setDialogType] = useState<DialogType>(null)
  const [detailLoading, setDetailLoading] = useState(false)
  
  // 分页状态（后端分页）
  const [currentPage, setCurrentPage] = useState(1)
  const [pageSize, setPageSize] = useState(20)
  const [totalItems, setTotalItems] = useState(0)
  const [totalPages, setTotalPages] = useState(0)

  // 获取仪表板统计数据
  const fetchDashboardStats = useCallback(async () => {
    try {
      const response = await api.logs.getDashboardStats()
      if (response.success && response.data) {
        setDashboardStats(response.data)
      }
    } catch (error) {
      console.error('获取仪表板统计数据失败:', error)
    }
  }, [])

  // 获取日志列表数据
  const fetchData = useCallback(async () => {
    try {
      setLoading(true)
      setError(null)
      
      const response = await api.logs.getList({
        page: currentPage,
        limit: pageSize,
        search: searchTerm || undefined,
        method: methodFilter === 'all' ? undefined : methodFilter,
        status_code: statusFilter === 'all' ? undefined : parseInt(statusFilter),
        user_service_api_name: userKeyNameFilter || undefined,
        user_provider_key_name: providerKeyNameFilter || undefined,
      })
      
      if (!response.success) {
        throw new Error(response.error?.message || '获取日志列表失败')
      }

      if (response.data && !Array.isArray(response.data)) {
        throw new Error('日志数据格式异常')
      }

      const traces = response.data ?? []
      setData(traces)

      if (response.pagination) {
        setTotalItems(response.pagination.total)
        setTotalPages(response.pagination.pages)
      } else {
        const fallbackTotal = traces.length
        setTotalItems(fallbackTotal)
        setTotalPages(Math.max(1, Math.ceil(fallbackTotal / pageSize)))
      }
    } catch (error) {
      console.error('获取日志列表失败:', error)
      setError(error instanceof Error ? error.message : '获取数据失败')
    } finally {
      setLoading(false)
    }
  }, [currentPage, pageSize, searchTerm, methodFilter, statusFilter, userKeyNameFilter, providerKeyNameFilter])

  // 打开详情弹窗并从后端获取完整数据
  const openDetailDialog = async (traceId: number) => {
    setDialogType('details')
    setDetailLoading(true)
    setSelectedItem(null)

    try {
      const response = await api.logs.getDetail(traceId)
      if (response.success && response.data) {
        setSelectedItem(response.data)
      } else {
        setError(response.error?.message || '获取日志详情失败')
        setDialogType(null)
      }
    } catch (error) {
      console.error('获取日志详情失败:', error)
      setError('获取日志详情失败')
      setDialogType(null)
    } finally {
      setDetailLoading(false)
    }
  }

  // 初始化数据加载
  useEffect(() => {
    fetchDashboardStats()
  }, [fetchDashboardStats])

  useEffect(() => {
    fetchData()
  }, [fetchData])

  // 由于后端已经处理了过滤和分页，前端直接使用返回的数据
  const paginatedData = data
  
  // 重置页码当过滤条件改变时
  React.useEffect(() => {
    setCurrentPage(1)
  }, [searchTerm, methodFilter, statusFilter, userKeyNameFilter, providerKeyNameFilter])

  // 格式化时间戳
  const formatTimestamp = (timestamp: string) => {
    const date = new Date(timestamp)
    return {
      date: date.toLocaleDateString(),
      time: date.toLocaleTimeString()
    }
  }

  // 渲染成功状态
  const renderSuccessStatus = (isSuccess: boolean) => {
    return isSuccess ? (
      <span className="inline-flex items-center px-2 py-1 rounded-full text-xs font-medium bg-emerald-50 text-emerald-700 ring-1 ring-emerald-200">
        <CheckCircle size={10} className="mr-1" />
        成功
      </span>
    ) : (
      <span className="inline-flex items-center px-2 py-1 rounded-full text-xs font-medium bg-red-50 text-red-700 ring-1 ring-red-200">
        <XCircle size={10} className="mr-1" />
        失败
      </span>
    )
  }

  // 渲染状态码
  const renderStatusCode = (statusCode: number) => {
    const isError = statusCode >= 400
    return (
      <span className={`px-2 py-1 rounded text-xs font-mono ${
        isError 
          ? 'bg-red-50 text-red-700 ring-1 ring-red-200' 
          : 'bg-emerald-50 text-emerald-700 ring-1 ring-emerald-200'
      }`}>
        {statusCode}
      </span>
    )
  }

  return (
    <div className="w-full">
      {/* 页面头部 */}
      <div className="flex items-center justify-between mb-6">
        <div>
          <h2 className="text-lg font-medium text-neutral-800">代理跟踪日志</h2>
          <p className="text-sm text-neutral-600 mt-1">查看和分析API代理请求的跟踪记录</p>
        </div>
        <div className="flex gap-2">
          <button
            onClick={() => {
              fetchData()
              fetchDashboardStats()
            }}
            disabled={loading}
            className="flex items-center gap-2 px-3 py-2 text-sm text-neutral-600 hover:text-neutral-800 disabled:opacity-50"
            title="刷新数据"
          >
            {loading ? <LoadingSpinner size="sm" tone="muted" /> : <RefreshCw size={16} />}
            刷新
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
      <div className="mb-6 grid grid-cols-1 md:grid-cols-4 gap-4">
        <StatCard
          icon={<FileText size={18} />}
          value={dashboardStats?.total_requests?.toString() || '0'}
          label="总请求数"
          color="#7c3aed"
        />
        <StatCard
          icon={<CheckCircle size={18} />}
          value={dashboardStats?.successful_requests?.toString() || '0'}
          label="成功请求"
          color="#10b981"
        />
        <StatCard
          icon={<XCircle size={18} />}
          value={dashboardStats?.failed_requests?.toString() || '0'}
          label="失败请求"
          color="#ef4444"
        />
        <StatCard
          icon={<Timer size={18} />}
          value={dashboardStats?.avg_response_time ? `${dashboardStats.avg_response_time}ms` : '0ms'}
          label="平均响应时间"
          color="#0ea5e9"
        />
      </div>

      {/* 搜索和过滤 */}
      <div className="flex items-center gap-4 mb-4">
        <div className="relative flex-1 max-w-md">
          <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 text-neutral-400" size={16} />
          <input
            type="text"
            placeholder="搜索路径、模型或错误信息..."
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            className="w-full pl-10 pr-4 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>
        <div className="flex items-center gap-4">
          <FilterSelect
            value={methodFilter}
            onValueChange={setMethodFilter}
            options={[
              { value: 'all', label: '全部方法' },
              { value: 'GET', label: 'GET' },
              { value: 'POST', label: 'POST' },
              { value: 'PUT', label: 'PUT' },
              { value: 'DELETE', label: 'DELETE' }
            ]}
            placeholder="全部方法"
          />
          <FilterSelect
            value={statusFilter}
            onValueChange={setStatusFilter}
            options={[
              { value: 'all', label: '全部状态' },
              { value: '200', label: '200 成功' },
              { value: '400', label: '400 错误请求' },
              { value: '401', label: '401 未授权' },
              { value: '403', label: '403 禁止访问' },
              { value: '404', label: '404 未找到' },
              { value: '500', label: '500 服务器错误' }
            ]}
            placeholder="全部状态"
          />
          <input
            value={userKeyNameFilter}
            onChange={(e) => setUserKeyNameFilter(e.target.value)}
            placeholder="用户 API Key 名称"
            className="w-48 px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
          <input
            value={providerKeyNameFilter}
            onChange={(e) => setProviderKeyNameFilter(e.target.value)}
            placeholder="账号 API Key 名称"
            className="w-48 px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>
      </div>

      {/* 数据表格 */}
      <div className="bg-white rounded-2xl border border-neutral-200 overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead className="bg-neutral-50 text-neutral-600">
              <tr>
                <th className="px-4 py-3 text-left font-medium">时间</th>
                <th className="px-4 py-3 text-left font-medium">用户 API Key</th>
                <th className="px-4 py-3 text-left font-medium">账号 API Key</th>
                <th className="px-4 py-3 text-left font-medium">路径</th>
                <th className="px-4 py-3 text-left font-medium">状态</th>
                <th className="px-4 py-3 text-left font-medium">模型</th>
                <th className="px-4 py-3 text-left font-medium">Token</th>
                <th className="px-4 py-3 text-left font-medium">费用</th>
                <th className="px-4 py-3 text-left font-medium">操作</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-neutral-200">
              {loading ? (
                <tr>
                  <td colSpan={9} className="px-4 py-8 text-center">
                    <div className="flex justify-center items-center gap-2">
                      <LoadingState text="加载中..." />
                    </div>
                  </td>
                </tr>
              ) : paginatedData.length === 0 ? (
                <tr>
                  <td colSpan={9} className="px-4 py-8 text-center text-neutral-500">
                    暂无数据
                  </td>
                </tr>
              ) : (
                paginatedData.map((item) => {
                  const { date, time } = formatTimestamp(item.created_at)
                  return (
                    <tr key={item.id} className="text-neutral-800 hover:bg-neutral-50">
                      <td className="px-4 py-3">
                        <div className="flex items-center gap-2">
                          <Calendar size={14} className="text-neutral-400" />
                          <div>
                            <div className="text-xs text-neutral-500">{date}</div>
                            <div className="text-xs font-mono text-neutral-700">{time}</div>
                          </div>
                        </div>
                      </td>
                      <td className="px-4 py-3">
                        <div className="flex flex-col text-xs text-neutral-600">
                          <span className="font-medium text-neutral-800">{item.user_service_api_name || '未命名'}</span>
                          <span className="text-neutral-500">ID: {item.user_service_api_id}</span>
                        </div>
                      </td>
                      <td className="px-4 py-3">
                        <div className="flex flex-col text-xs text-neutral-600">
                          <span className="font-medium text-neutral-800">{item.user_provider_key_name || '未绑定'}</span>
                          {item.user_provider_key_id && (
                            <span className="text-neutral-500">ID: {item.user_provider_key_id}</span>
                          )}
                        </div>
                      </td>
                      <td className="px-4 py-3">
                        <code className="text-xs bg-neutral-100 px-2 py-1 rounded max-w-xs truncate block">
                          {item.path || 'N/A'}
                        </code>
                      </td>
                      <td className="px-4 py-3">
                        <div className="flex items-center gap-2">
                          {renderSuccessStatus(item.is_success)}
                          {item.status_code && renderStatusCode(item.status_code)}
                        </div>
                      </td>
                      <td className="px-4 py-3">
                        <span className="text-xs text-neutral-600">
                          {item.model_used || 'N/A'}
                        </span>
                      </td>
                      <td className="px-4 py-3">
                        <div className="text-xs space-y-1">
                          <div className="font-medium">总计: {item.tokens_total.toLocaleString()}</div>
                          <div className="text-neutral-500 space-y-0.5">
                            <div>输入: {item.tokens_prompt.toLocaleString()} | 输出: {item.tokens_completion.toLocaleString()}</div>
                            <div>缓存创建: {item.cache_create_tokens.toLocaleString()} | 缓存读取: {item.cache_read_tokens.toLocaleString()}</div>
                          </div>
                        </div>
                      </td>
                      <td className="px-4 py-3">
                        <div className="text-xs">
                          {item.cost ? `$${item.cost.toFixed(4)}` : 'N/A'}
                          {item.cost_currency && item.cost_currency !== 'USD' && (
                            <span className="text-neutral-500"> {item.cost_currency}</span>
                          )}
                        </div>
                      </td>
                      <td className="px-4 py-3">
                        <button
                          onClick={() => openDetailDialog(item.id)}
                          className="p-1 text-neutral-500 hover:text-violet-600"
                          title="查看详情"
                        >
                          <Eye size={16} />
                        </button>
                      </td>
                    </tr>
                  )
                })
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


      {/* 详情对话框 */}
      {dialogType === 'details' && (
        <LogDetailsDialog
          item={selectedItem}
          loading={detailLoading}
          onClose={() => {
            setDialogType(null)
            setSelectedItem(null)
            setDetailLoading(false)
          }}
        />
      )}
    </div>
  )
}

/** 代理跟踪日志详情对话框 */
const LogDetailsDialog: React.FC<{
  item: ProxyTraceEntry | null
  loading: boolean
  onClose: () => void
}> = ({ item, loading, onClose }) => {
  const { date, time } = React.useMemo(() => {
    if (!item) {
      return { date: '', time: '' }
    }
    const d = new Date(item.created_at)
    return {
      date: d.toLocaleDateString(),
      time: d.toLocaleTimeString()
    }
  }, [item])

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-white rounded-2xl p-6 w-full max-w-2xl mx-4 max-h-[80vh] overflow-y-auto">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-lg font-medium text-neutral-900">代理跟踪日志详情</h3>
          <button
            onClick={onClose}
            className="text-neutral-500 hover:text-neutral-700"
          >
            ×
          </button>
        </div>
        
        {loading ? (
          <div className="py-12 flex items-center justify-center text-neutral-500">
            <LoadingSpinner size="lg" tone="muted" />
          </div>
        ) : !item ? (
          <div className="py-12 text-center text-neutral-500">暂无详情数据</div>
        ) : (
        <div className="space-y-4">
          <div className="grid grid-cols-2 gap-4">
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">请求ID</div>
              <div className="font-mono text-sm">{item.request_id}</div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">创建时间</div>
              <div className="text-sm">{date} {time}</div>
            </div>
          </div>

          <div className="grid grid-cols-3 gap-4">
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">HTTP方法</div>
              <div className="font-medium">{item.method}</div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">状态码</div>
              <div className="font-medium">{item.status_code || 'N/A'}</div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">执行状态</div>
              <div className="font-medium">{item.is_success ? '成功' : '失败'}</div>
            </div>
          </div>

          <div className="p-3 bg-neutral-50 rounded-lg">
            <div className="text-sm text-neutral-600">请求路径</div>
            <code className="text-sm bg-neutral-100 px-2 py-1 rounded mt-1 inline-block">
              {item.path || 'N/A'}
            </code>
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">用户 API Key</div>
              <div className="font-medium">
                {item.user_service_api_name || '未命名'}
              </div>
              <div className="text-xs text-neutral-500 mt-1">
                ID: {item.user_service_api_id}
              </div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">账号 API Key</div>
              <div className="font-medium">
                {item.user_provider_key_name || '未绑定'}
              </div>
              <div className="text-xs text-neutral-500 mt-1">
                ID: {item.user_provider_key_id || 'N/A'}
              </div>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">使用模型</div>
              <div className="font-medium">{item.model_used || 'N/A'}</div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">服务商</div>
              <div className="font-medium">{item.provider_name || 'N/A'}</div>
            </div>
          </div>

          <div className="p-3 bg-neutral-50 rounded-lg">
            <div className="text-sm text-neutral-600 mb-3">Token使用详情</div>
            <div className="grid grid-cols-2 gap-4">
              <div>
                <div className="text-xs text-neutral-500">总Token数量</div>
                <div className="font-medium text-lg text-violet-600">{item.tokens_total.toLocaleString()}</div>
              </div>
              <div>
                <div className="text-xs text-neutral-500">效率比率</div>
                <div className="font-medium">{item.token_efficiency_ratio?.toFixed(2) || 'N/A'}</div>
              </div>
            </div>
            <div className="mt-3 grid grid-cols-2 gap-3 text-sm">
              <div className="flex justify-between">
                <span className="text-neutral-600">输入Token:</span>
                <span className="font-medium">{item.tokens_prompt.toLocaleString()}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-neutral-600">输出Token:</span>
                <span className="font-medium">{item.tokens_completion.toLocaleString()}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-neutral-600">缓存创建:</span>
                <span className="font-medium">{item.cache_create_tokens.toLocaleString()}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-neutral-600">缓存读取:</span>
                <span className="font-medium">{item.cache_read_tokens.toLocaleString()}</span>
              </div>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">费用</div>
              <div className="font-medium">{item.cost ? `$${item.cost.toFixed(4)}` : 'N/A'} {item.cost_currency}</div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">重试次数</div>
              <div className="font-medium">{item.retry_count}</div>
            </div>
          </div>

          {(item.start_time || item.end_time || item.duration_ms) && (
            <div className="grid grid-cols-3 gap-4">
              {item.start_time && (
                <div className="p-3 bg-neutral-50 rounded-lg">
                  <div className="text-sm text-neutral-600">开始时间</div>
                  <div className="text-xs font-mono">{new Date(item.start_time).toLocaleString()}</div>
                </div>
              )}
              {item.end_time && (
                <div className="p-3 bg-neutral-50 rounded-lg">
                  <div className="text-sm text-neutral-600">结束时间</div>
                  <div className="text-xs font-mono">{new Date(item.end_time).toLocaleString()}</div>
                </div>
              )}
              {item.duration_ms && (
                <div className="p-3 bg-neutral-50 rounded-lg">
                  <div className="text-sm text-neutral-600">执行时长</div>
                  <div className="font-medium">{item.duration_ms}ms</div>
                </div>
              )}
            </div>
          )}

          <div className="grid grid-cols-2 gap-4">
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">客户端IP</div>
              <div className="font-mono text-sm">{item.client_ip || 'N/A'}</div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">用户代理</div>
              <div className="text-xs truncate max-w-xs" title={item.user_agent || 'N/A'}>
                {item.user_agent || 'N/A'}
              </div>
            </div>
          </div>

          {(item.error_type || item.error_message) && (
            <div className="p-3 bg-red-50 rounded-lg border border-red-200">
              <div className="text-sm text-red-600 font-medium mb-2">错误信息</div>
              {item.error_type && (
                <div className="mb-1">
                  <span className="text-xs text-red-500">类型: </span>
                  <span className="text-sm font-mono">{item.error_type}</span>
                </div>
              )}
              {item.error_message && (() => {
                const parsed = tryParseErrorPayload(item.error_message)
                return parsed ? (
                  <div className="mt-2">
                    <div className="text-xs text-red-500 mb-1">详细信息:</div>
                    <pre className="max-h-64 overflow-auto rounded bg-neutral-900 text-neutral-50 text-xs leading-relaxed p-3 whitespace-pre-wrap">
                      {JSON.stringify(parsed, null, 2)}
                    </pre>
                  </div>
                ) : (
                  <div>
                    <span className="text-xs text-red-500">消息: </span>
                    <span className="text-sm">{item.error_message}</span>
                  </div>
                )
              })()}
            </div>
          )}
        </div>
        )}
      </div>
    </div>
  )
}

export default LogsPage

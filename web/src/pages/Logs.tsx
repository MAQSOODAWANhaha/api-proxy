/**
 * Logs.tsx
 * 请求记录页面：完整的请求记录数据展示、搜索过滤和分页功能
 */

import React, { useState, useMemo } from 'react'
import {
  Search,
  Filter,
  RefreshCw,
  Calendar,
  Clock,
  AlertTriangle,
  Info,
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

/** 请求记录级别类型 */
type LogLevel = 'info' | 'warn' | 'error' | 'success'

/** 请求记录数据结构 */
interface LogEntry {
  id: string
  timestamp: string
  level: LogLevel
  service: string
  endpoint: string
  message: string
  userId?: string
  ip: string
  duration: number
  statusCode: number
  requestId: string
}

/** 模拟请求记录数据 */
const generateMockLogs = (): LogEntry[] => {
  const services = ['api-gateway', 'auth-service', 'user-service', 'ai-service', 'billing-service']
  const endpoints = ['/api/chat/completions', '/api/auth/login', '/api/users/profile', '/api/keys/create', '/api/billing/usage']
  const levels: LogLevel[] = ['info', 'warn', 'error', 'success']
  const messages = {
    info: ['Request processed successfully', 'User authenticated', 'API key validated', 'Cache hit'],
    warn: ['Rate limit approaching', 'Slow query detected', 'Deprecated API used', 'High memory usage'],
    error: ['Authentication failed', 'Database connection error', 'Invalid API key', 'Request timeout'],
    success: ['Payment processed', 'User created', 'API key generated', 'Export completed']
  }

  const logs: LogEntry[] = []
  for (let i = 0; i < 150; i++) {
    const level = levels[Math.floor(Math.random() * levels.length)]
    const service = services[Math.floor(Math.random() * services.length)]
    const endpoint = endpoints[Math.floor(Math.random() * endpoints.length)]
    const messageArray = messages[level]
    const message = messageArray[Math.floor(Math.random() * messageArray.length)]
    
    const date = new Date()
    date.setMinutes(date.getMinutes() - Math.floor(Math.random() * 10080)) // 过去7天内
    
    logs.push({
      id: `log-${i + 1}`,
      timestamp: date.toISOString(),
      level,
      service,
      endpoint,
      message,
      userId: Math.random() > 0.3 ? `user-${Math.floor(Math.random() * 100)}` : undefined,
      ip: `192.168.1.${Math.floor(Math.random() * 255)}`,
      duration: Math.floor(Math.random() * 2000) + 50,
      statusCode: level === 'error' ? [400, 401, 403, 404, 500][Math.floor(Math.random() * 5)] : 
                  level === 'warn' ? [429, 503][Math.floor(Math.random() * 2)] : 200,
      requestId: `req-${Math.random().toString(36).substring(2, 12)}`
    })
  }

  return logs.sort((a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime())
}

const initialData = generateMockLogs()

/** 弹窗类型 */
type DialogType = 'details' | null

/** 页面主组件 */
const LogsPage: React.FC = () => {
  const [data] = useState<LogEntry[]>(initialData)
  const [searchTerm, setSearchTerm] = useState('')
  const [levelFilter, setLevelFilter] = useState<'all' | LogLevel>('all')
  const [serviceFilter, setServiceFilter] = useState<string>('all')
  const [selectedItem, setSelectedItem] = useState<LogEntry | null>(null)
  const [dialogType, setDialogType] = useState<DialogType>(null)
  
  // 分页状态
  const [currentPage, setCurrentPage] = useState(1)
  const [pageSize, setPageSize] = useState(20)

  // 获取所有服务列表
  const services = useMemo(() => {
    const uniqueServices = Array.from(new Set(data.map(item => item.service)))
    return uniqueServices
  }, [data])

  // 过滤数据
  const filteredData = useMemo(() => {
    return data.filter((item) => {
      const matchesSearch = 
        item.message.toLowerCase().includes(searchTerm.toLowerCase()) ||
        item.endpoint.toLowerCase().includes(searchTerm.toLowerCase()) ||
        item.requestId.toLowerCase().includes(searchTerm.toLowerCase()) ||
        (item.userId && item.userId.toLowerCase().includes(searchTerm.toLowerCase()))
      const matchesLevel = levelFilter === 'all' || item.level === levelFilter
      const matchesService = serviceFilter === 'all' || item.service === serviceFilter
      return matchesSearch && matchesLevel && matchesService
    })
  }, [data, searchTerm, levelFilter, serviceFilter])

  // 分页数据和计算
  const paginatedData = useMemo(() => {
    const startIndex = (currentPage - 1) * pageSize
    return filteredData.slice(startIndex, startIndex + pageSize)
  }, [filteredData, currentPage, pageSize])

  const totalPages = Math.ceil(filteredData.length / pageSize)
  
  // 重置页码当过滤条件改变时
  React.useEffect(() => {
    setCurrentPage(1)
  }, [searchTerm, levelFilter, serviceFilter])

  // 格式化时间戳
  const formatTimestamp = (timestamp: string) => {
    const date = new Date(timestamp)
    return {
      date: date.toLocaleDateString(),
      time: date.toLocaleTimeString()
    }
  }

  // 渲染请求记录级别
  const renderLogLevel = (level: LogLevel) => {
    const levelConfig = {
      info: { 
        icon: Info, 
        color: 'text-blue-600', 
        bg: 'bg-blue-50', 
        ring: 'ring-blue-200', 
        text: 'INFO' 
      },
      warn: { 
        icon: AlertTriangle, 
        color: 'text-yellow-600', 
        bg: 'bg-yellow-50', 
        ring: 'ring-yellow-200', 
        text: 'WARN' 
      },
      error: { 
        icon: XCircle, 
        color: 'text-red-600', 
        bg: 'bg-red-50', 
        ring: 'ring-red-200', 
        text: 'ERROR' 
      },
      success: { 
        icon: CheckCircle, 
        color: 'text-emerald-600', 
        bg: 'bg-emerald-50', 
        ring: 'ring-emerald-200', 
        text: 'SUCCESS' 
      },
    }
    const config = levelConfig[level]
    const IconComponent = config.icon
    
    return (
      <span className={`inline-flex items-center px-2 py-1 rounded-full text-xs font-medium ${config.bg} ${config.color} ring-1 ${config.ring}`}>
        <IconComponent size={10} className="mr-1" />
        {config.text}
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
          <h2 className="text-lg font-medium text-neutral-800">请求记录</h2>
          <p className="text-sm text-neutral-600 mt-1">查看和分析API请求记录</p>
        </div>
        <div className="flex gap-2">
          <button
            onClick={() => window.location.reload()}
            className="flex items-center gap-2 px-3 py-2 text-sm text-neutral-600 hover:text-neutral-800"
            title="刷新数据"
          >
            <RefreshCw size={16} />
            刷新
          </button>
        </div>
      </div>

      {/* 统计信息 */}
      <div className="mb-6 grid grid-cols-1 md:grid-cols-4 gap-4">
        <StatCard
          icon={<FileText size={18} />}
          value={data.length.toString()}
          label="总记录数"
          color="#7c3aed"
        />
        <StatCard
          icon={<XCircle size={18} />}
          value={data.filter(item => item.level === 'error').length.toString()}
          label="错误记录"
          color="#ef4444"
        />
        <StatCard
          icon={<AlertTriangle size={18} />}
          value={data.filter(item => item.level === 'warn').length.toString()}
          label="警告记录"
          color="#f59e0b"
        />
        <StatCard
          icon={<Timer size={18} />}
          value={`${Math.round(data.reduce((sum, item) => sum + item.duration, 0) / data.length)}ms`}
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
            placeholder="搜索消息、接口、请求ID或用户..."
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            className="w-full pl-10 pr-4 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>
        <div className="flex items-center gap-4">
          <FilterSelect
            value={serviceFilter}
            onValueChange={setServiceFilter}
            options={[
              { value: 'all', label: '全部服务' },
              ...services.map(service => ({
                value: service,
                label: service
              }))
            ]}
            placeholder="全部服务"
          />
          <FilterSelect
            value={levelFilter}
            onValueChange={(value) => setLevelFilter(value as 'all' | LogLevel)}
            options={[
              { value: 'all', label: '全部级别' },
              { value: 'info', label: 'INFO' },
              { value: 'warn', label: 'WARN' },
              { value: 'error', label: 'ERROR' },
              { value: 'success', label: 'SUCCESS' }
            ]}
            placeholder="全部级别"
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
                <th className="px-4 py-3 text-left font-medium">级别</th>
                <th className="px-4 py-3 text-left font-medium">服务</th>
                <th className="px-4 py-3 text-left font-medium">接口</th>
                <th className="px-4 py-3 text-left font-medium">消息</th>
                <th className="px-4 py-3 text-left font-medium">状态码</th>
                <th className="px-4 py-3 text-left font-medium">耗时</th>
                <th className="px-4 py-3 text-left font-medium">操作</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-neutral-200">
              {paginatedData.map((item) => {
                const { date, time } = formatTimestamp(item.timestamp)
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
                    <td className="px-4 py-3">{renderLogLevel(item.level)}</td>
                    <td className="px-4 py-3">
                      <span className="px-2 py-1 bg-neutral-100 text-neutral-700 rounded text-xs font-mono">
                        {item.service}
                      </span>
                    </td>
                    <td className="px-4 py-3">
                      <code className="text-xs bg-neutral-100 px-2 py-1 rounded">
                        {item.endpoint}
                      </code>
                    </td>
                    <td className="px-4 py-3">
                      <div className="max-w-xs truncate" title={item.message}>
                        {item.message}
                      </div>
                    </td>
                    <td className="px-4 py-3">{renderStatusCode(item.statusCode)}</td>
                    <td className="px-4 py-3">
                      <div className="flex items-center gap-1">
                        <Clock size={12} className="text-neutral-400" />
                        <span className="text-xs">{item.duration}ms</span>
                      </div>
                    </td>
                    <td className="px-4 py-3">
                      <button
                        onClick={() => {
                          setSelectedItem(item)
                          setDialogType('details')
                        }}
                        className="p-1 text-neutral-500 hover:text-violet-600"
                        title="查看详情"
                      >
                        <Eye size={16} />
                      </button>
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        </div>
        
        {/* 分页组件 */}
        {totalPages > 1 && (
          <div className="flex items-center justify-between px-4 py-3 border-t border-neutral-200">
            <div className="text-sm text-neutral-600">
              显示 {(currentPage - 1) * pageSize + 1} - {Math.min(currentPage * pageSize, filteredData.length)} 条，共 {filteredData.length} 条记录
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
      {dialogType === 'details' && selectedItem && (
        <LogDetailsDialog 
          item={selectedItem} 
          onClose={() => {
            setDialogType(null)
            setSelectedItem(null)
          }} 
        />
      )}
    </div>
  )
}

/** 请求记录详情对话框 */
const LogDetailsDialog: React.FC<{
  item: LogEntry
  onClose: () => void
}> = ({ item, onClose }) => {
  const { date, time } = React.useMemo(() => {
    const d = new Date(item.timestamp)
    return {
      date: d.toLocaleDateString(),
      time: d.toLocaleTimeString()
    }
  }, [item.timestamp])

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-white rounded-2xl p-6 w-full max-w-2xl mx-4 max-h-[80vh] overflow-y-auto">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-lg font-medium text-neutral-900">请求记录详情</h3>
          <button
            onClick={onClose}
            className="text-neutral-500 hover:text-neutral-700"
          >
            ×
          </button>
        </div>
        
        <div className="space-y-4">
          <div className="grid grid-cols-2 gap-4">
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">请求ID</div>
              <div className="font-mono text-sm">{item.requestId}</div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">时间戳</div>
              <div className="text-sm">{date} {time}</div>
            </div>
          </div>

          <div className="grid grid-cols-3 gap-4">
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">服务</div>
              <div className="font-medium">{item.service}</div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">状态码</div>
              <div className="font-medium">{item.statusCode}</div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">耗时</div>
              <div className="font-medium">{item.duration}ms</div>
            </div>
          </div>

          <div className="p-3 bg-neutral-50 rounded-lg">
            <div className="text-sm text-neutral-600">接口</div>
            <code className="text-sm bg-neutral-100 px-2 py-1 rounded mt-1 inline-block">
              {item.endpoint}
            </code>
          </div>

          <div className="p-3 bg-neutral-50 rounded-lg">
            <div className="text-sm text-neutral-600">消息</div>
            <div className="mt-1">{item.message}</div>
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">用户ID</div>
              <div className="font-mono text-sm">{item.userId || 'N/A'}</div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">IP地址</div>
              <div className="font-mono text-sm">{item.ip}</div>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}

export default LogsPage
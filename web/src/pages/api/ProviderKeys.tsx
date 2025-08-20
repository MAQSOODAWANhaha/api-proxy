/**
 * ProviderKeys.tsx
 * 账号（上游服务商）API Keys 管理页：完整的增删改查和统计功能
 */

import React, { useState, useMemo } from 'react'
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
import { api } from '../../lib/api'

/** 账号 API Key 数据结构 */
interface ProviderKey {
  id: string
  provider: string
  keyName: string
  keyValue: string
  weight: number
  requestLimitPerMinute: number
  tokenLimitPromptPerMinute: number
  requestLimitPerDay: number
  status: 'active' | 'disabled' | 'error'
  usage: number
  cost: number
  createdAt: string
  healthCheck: 'healthy' | 'warning' | 'error'
}

/** 服务商类型 */
interface ProviderType {
  id: number
  name: string
  display_name: string
  description: string
  is_active: boolean
  supported_models: string[]
  created_at: string
}

/** 模拟数据 */
const initialData: ProviderKey[] = [
  {
    id: '1',
    provider: 'OpenAI',
    keyName: 'Primary GPT Key',
    keyValue: 'sk-1234567890abcdef1234567890abcdef',
    weight: 1,
    requestLimitPerMinute: 60,
    tokenLimitPromptPerMinute: 1000,
    requestLimitPerDay: 10000,
    status: 'active',
    usage: 8520,
    cost: 125.50,
    createdAt: '2024-01-10',
    healthCheck: 'healthy',
  },
  {
    id: '2',
    provider: 'Anthropic',
    keyName: 'Claude Production',
    keyValue: 'sk-ant-abcdef1234567890abcdef1234567890',
    weight: 2,
    requestLimitPerMinute: 30,
    tokenLimitPromptPerMinute: 800,
    requestLimitPerDay: 5000,
    status: 'active',
    usage: 3420,
    cost: 89.30,
    createdAt: '2024-01-12',
    healthCheck: 'healthy',
  },
  {
    id: '3',
    provider: 'Google',
    keyName: 'Gemini Backup',
    keyValue: 'AIzaSyDxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx',
    weight: 3,
    requestLimitPerMinute: 20,
    tokenLimitPromptPerMinute: 500,
    requestLimitPerDay: 2500,
    status: 'disabled',
    usage: 145,
    cost: 12.80,
    createdAt: '2024-01-08',
    healthCheck: 'error',
  },
]

/** 弹窗类型 */
type DialogType = 'add' | 'edit' | 'delete' | 'stats' | null

/** 页面主组件 */
const ProviderKeysPage: React.FC = () => {
  const [data, setData] = useState<ProviderKey[]>(initialData)
  const [searchTerm, setSearchTerm] = useState('')
  const [statusFilter, setStatusFilter] = useState<'all' | 'active' | 'disabled' | 'error'>('all')
  const [providerFilter, setProviderFilter] = useState<string>('all')
  const [selectedItem, setSelectedItem] = useState<ProviderKey | null>(null)
  const [dialogType, setDialogType] = useState<DialogType>(null)
  const [showKeyValues, setShowKeyValues] = useState<{ [key: string]: boolean }>({})
  
  // 分页状态
  const [currentPage, setCurrentPage] = useState(1)
  const [pageSize, setPageSize] = useState(10)

  // 获取所有账号列表
  const providers = useMemo(() => {
    const uniqueProviders = Array.from(new Set(data.map(item => item.provider)))
    return uniqueProviders
  }, [data])

  // 过滤数据
  const filteredData = useMemo(() => {
    return data.filter((item) => {
      const matchesSearch = 
        item.provider.toLowerCase().includes(searchTerm.toLowerCase()) ||
        item.keyName.toLowerCase().includes(searchTerm.toLowerCase())
      const matchesStatus = statusFilter === 'all' || item.status === statusFilter
      const matchesProvider = providerFilter === 'all' || item.provider === providerFilter
      return matchesSearch && matchesStatus && matchesProvider
    })
  }, [data, searchTerm, statusFilter, providerFilter])

  // 分页数据和计算
  const paginatedData = useMemo(() => {
    const startIndex = (currentPage - 1) * pageSize
    return filteredData.slice(startIndex, startIndex + pageSize)
  }, [filteredData, currentPage, pageSize])

  const totalPages = Math.ceil(filteredData.length / pageSize)
  
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
  const handleAdd = (newKey: Omit<ProviderKey, 'id' | 'usage' | 'cost' | 'createdAt' | 'healthCheck' | 'keyValue'>) => {
    const providerKey: ProviderKey = {
      ...newKey,
      id: Date.now().toString(),
      usage: 0,
      cost: 0,
      createdAt: new Date().toISOString().split('T')[0],
      keyValue: generateApiKey(newKey.provider),
      healthCheck: 'healthy',
    }
    setData([...data, providerKey])
    setDialogType(null)
  }

  // 编辑API Key
  const handleEdit = (updatedKey: ProviderKey) => {
    setData(data.map(item => item.id === updatedKey.id ? updatedKey : item))
    setDialogType(null)
    setSelectedItem(null)
  }

  // 删除API Key
  const handleDelete = () => {
    if (selectedItem) {
      setData(data.filter(item => item.id !== selectedItem.id))
      setDialogType(null)
      setSelectedItem(null)
    }
  }

  // 健康检查
  const performHealthCheck = async (id: string) => {
    setData(data.map(item => 
      item.id === id 
        ? { ...item, healthCheck: Math.random() > 0.3 ? 'healthy' : 'warning' as const }
        : item
    ))
  }

  // 切换API Key可见性
  const toggleKeyVisibility = (id: string) => {
    setShowKeyValues(prev => ({ ...prev, [id]: !prev[id] }))
  }

  // 复制到剪贴板
  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text)
  }

  // 渲染健康状态
  const renderHealthStatus = (status: string) => {
    const statusConfig = {
      healthy: { color: 'text-emerald-600', bg: 'bg-emerald-50', ring: 'ring-emerald-200', text: '正常' },
      warning: { color: 'text-yellow-600', bg: 'bg-yellow-50', ring: 'ring-yellow-200', text: '警告' },
      error: { color: 'text-red-600', bg: 'bg-red-50', ring: 'ring-red-200', text: '异常' },
    }
    const config = statusConfig[status as keyof typeof statusConfig]
    
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
            onClick={() => setData([...initialData])}
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
          value={data.length.toString()}
          label="总密钥数"
          color="#7c3aed"
        />
        <StatCard
          icon={<Activity size={18} />}
          value={data.filter(item => item.status === 'active').length.toString()}
          label="活跃密钥"
          color="#10b981"
        />
        <StatCard
          icon={<BarChart3 size={18} />}
          value={data.reduce((sum, item) => sum + item.usage, 0).toLocaleString()}
          label="总使用次数"
          color="#0ea5e9"
        />
        <StatCard
          icon={<DollarSign size={18} />}
          value={`$${data.reduce((sum, item) => sum + item.cost, 0).toFixed(2)}`}
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
            onValueChange={(value) => setStatusFilter(value as 'all' | 'active' | 'disabled' | 'error')}
            options={[
              { value: 'all', label: '全部状态' },
              { value: 'active', label: '启用' },
              { value: 'disabled', label: '停用' },
              { value: 'error', label: '异常' }
            ]}
            placeholder="全部状态"
          />
        </div>
      </div>

      {/* 数据表格 */}
      <div className="bg-white rounded-2xl border border-neutral-200 overflow-hidden">
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
                  <td className="px-4 py-3">{renderMaskedKey(item.keyValue, item.id)}</td>
                  <td className="px-4 py-3">
                    <div className="flex items-center gap-2">
                      <span className="text-sm">{item.usage.toLocaleString()}</span>
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
                    <div className="text-sm font-medium text-neutral-900">${item.cost.toFixed(2)}</div>
                    <div className="text-xs text-neutral-500">本月花费</div>
                  </td>
                  <td className="px-4 py-3">
                    <div className="flex items-center gap-2">
                      {renderHealthStatus(item.healthCheck)}
                      <button
                        onClick={() => performHealthCheck(item.id)}
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
              显示 {(currentPage - 1) * pageSize + 1} - {Math.min(currentPage * pageSize, filteredData.length)} 条，共 {filteredData.length} 条记录
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
        />
      )}
    </div>
  )
}

/** 对话框门户组件 */
const DialogPortal: React.FC<{
  type: DialogType
  selectedItem: ProviderKey | null
  onClose: () => void
  onAdd: (item: Omit<ProviderKey, 'id' | 'usage' | 'cost' | 'createdAt' | 'lastUsed' | 'healthCheck'>) => void
  onEdit: (item: ProviderKey) => void
  onDelete: () => void
}> = ({ type, selectedItem, onClose, onAdd, onEdit, onDelete }) => {
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
  onSubmit: (item: Omit<ProviderKey, 'id' | 'usage' | 'cost' | 'createdAt' | 'healthCheck' | 'keyValue'>) => void
}> = ({ onClose, onSubmit }) => {
  const [formData, setFormData] = useState({
    provider: '',
    keyName: '',
    weight: 1,
    requestLimitPerMinute: 0,
    tokenLimitPromptPerMinute: 0,
    requestLimitPerDay: 0,
    status: 'active' as 'active' | 'disabled',
  })

  // 服务商类型状态管理
  const [providerTypes, setProviderTypes] = useState<ProviderType[]>([])
  const [loadingProviderTypes, setLoadingProviderTypes] = useState(true)

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
            provider: firstProvider.display_name
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

  // 初始化：获取服务商类型
  React.useEffect(() => {
    fetchProviderTypes()
  }, [])

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
    <div className="bg-white rounded-2xl p-6 w-full max-w-lg mx-4 max-h-[80vh] overflow-y-auto">
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
              onValueChange={(value) => {
                const selectedProvider = providerTypes.find(type => type.display_name === value)
                setFormData({ 
                  ...formData, 
                  provider: selectedProvider ? selectedProvider.display_name : value
                })
              }}
              options={providerTypes.map(type => ({
                value: type.display_name,
                label: type.display_name
              }))}
              placeholder="请选择服务商类型"
            />
          )}
        </div>

        {/* API密钥 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">
            <span className="text-red-500">*</span> API密钥
          </label>
          <input
            type="text"
            placeholder="请输入API密钥"
            className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>

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
  item: ProviderKey
  onClose: () => void
  onSubmit: (item: ProviderKey) => void
}> = ({ item, onClose, onSubmit }) => {
  const [formData, setFormData] = useState({ ...item })

  // 服务商类型状态管理
  const [providerTypes, setProviderTypes] = useState<ProviderType[]>([])
  const [loadingProviderTypes, setLoadingProviderTypes] = useState(true)

  // 获取服务商类型列表
  const fetchProviderTypes = async () => {
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

  // 初始化：获取服务商类型
  React.useEffect(() => {
    fetchProviderTypes()
  }, [])

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
    <div className="bg-white rounded-2xl p-6 w-full max-w-lg mx-4 max-h-[80vh] overflow-y-auto">
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
              onValueChange={(value) => {
                const selectedProvider = providerTypes.find(type => type.display_name === value)
                setFormData({ 
                  ...formData, 
                  provider: selectedProvider ? selectedProvider.display_name : value
                })
              }}
              options={providerTypes.map(type => ({
                value: type.display_name,
                label: type.display_name
              }))}
              placeholder="请选择服务商类型"
            />
          )}
        </div>

        {/* API密钥 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">
            <span className="text-red-500">*</span> API密钥
          </label>
          <input
            type="text"
            value={formData.keyValue}
            onChange={(e) => setFormData({ ...formData, keyValue: e.target.value })}
            className="w-full px-3 py-2 border border-neutral-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>

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

        {/* Token限制/天 */}
        <div>
          <label className="block text-sm font-medium text-neutral-700 mb-1">Token限制/天</label>
          <div className="flex items-center">
            <button
              type="button"
              onClick={() => handleNumberChange('tokenLimitPerDay', -1)}
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
              onClick={() => handleNumberChange('tokenLimitPerDay', 1)}
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
  item: ProviderKey
  onClose: () => void
  onConfirm: () => void
}> = ({ item, onClose, onConfirm }) => {
  return (
    <div className="bg-white rounded-2xl p-6 w-full max-w-md mx-4">
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
const StatsDialog: React.FC<{
  item: ProviderKey
  onClose: () => void
}> = ({ item, onClose }) => {
  // 模拟统计数据
  const mockStats = {
    dailyUsage: [320, 450, 289, 645, 378, 534, 489],
    dailyCost: [12.5, 18.2, 11.3, 25.8, 15.1, 21.4, 19.6],
    successRate: 99.2,
    avgResponseTime: 850,
  }

  return (
    <div className="bg-white rounded-2xl p-6 w-full max-w-3xl mx-4 max-h-[80vh] overflow-y-auto">
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
            <div className="text-2xl font-bold text-violet-900">{item.usage.toLocaleString()}</div>
          </div>
          <div className="p-4 bg-orange-50 rounded-xl">
            <div className="text-sm text-orange-600">本月花费</div>
            <div className="text-2xl font-bold text-orange-900">${item.cost.toFixed(2)}</div>
          </div>
          <div className="p-4 bg-emerald-50 rounded-xl">
            <div className="text-sm text-emerald-600">成功率</div>
            <div className="text-2xl font-bold text-emerald-900">{mockStats.successRate}%</div>
          </div>
          <div className="p-4 bg-blue-50 rounded-xl">
            <div className="text-sm text-blue-600">平均响应时间</div>
            <div className="text-2xl font-bold text-blue-900">{mockStats.avgResponseTime}ms</div>
          </div>
        </div>

        {/* 使用趋势 */}
        <div className="grid grid-cols-2 gap-6">
          <div>
            <h4 className="text-sm font-medium text-neutral-900 mb-3">7天使用趋势</h4>
            <div className="flex items-end gap-1 h-32">
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
          
          <div>
            <h4 className="text-sm font-medium text-neutral-900 mb-3">7天花费趋势</h4>
            <div className="flex items-end gap-1 h-32">
              {mockStats.dailyCost.map((value, index) => (
                <div key={index} className="flex-1 flex flex-col items-center">
                  <div
                    className="w-full bg-orange-600 rounded-t"
                    style={{ height: `${(value / Math.max(...mockStats.dailyCost)) * 100}%` }}
                  />
                  <div className="text-xs text-neutral-500 mt-1">${value}</div>
                </div>
              ))}
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
              <div className="font-medium">{item.tokenLimitPerDay.toLocaleString()} Token/天</div>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}

export default ProviderKeysPage

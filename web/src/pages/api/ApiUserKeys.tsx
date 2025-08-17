/**
 * ApiUserKeys.tsx
 * 用户 API Keys 管理页：完整的增删改查和统计功能
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

/** API Key 数据结构 */
interface ApiKey {
  id: string
  keyName: string
  keyValue: string
  description: string
  providerType: string
  schedulingStrategy: 'round_robin' | 'priority' | 'weighted' | 'random'
  providerKeys: string[]
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

/** 模拟数据 */
const initialData: ApiKey[] = [
  {
    id: '1',
    keyName: 'Production Key',
    keyValue: 'sk-1234567890abcdef1234567890abcdef',
    description: '生产环境主要API密钥，用于客户端调用',
    providerType: 'OpenAI',
    schedulingStrategy: 'round_robin',
    providerKeys: ['openai-primary', 'openai-backup'],
    retryCount: 3,
    timeoutSeconds: 30,
    rateLimitPerMinute: 60,
    tokenLimitPerDay: 100000,
    status: 'active',
    usage: 1520,
    limit: 10000,
    createdAt: '2024-01-15',
    lastUsed: '2024-01-16 14:32',
  },
  {
    id: '2',
    keyName: 'Development Key',
    keyValue: 'sk-abcdef1234567890abcdef1234567890',
    description: '开发测试专用密钥',
    providerType: 'Anthropic',
    schedulingStrategy: 'priority',
    providerKeys: ['claude-dev'],
    retryCount: 2,
    timeoutSeconds: 15,
    rateLimitPerMinute: 30,
    tokenLimitPerDay: 10000,
    status: 'disabled',
    usage: 245,
    limit: 1000,
    createdAt: '2024-01-10',
    lastUsed: '2024-01-12 09:15',
  },
  {
    id: '3',
    keyName: 'Testing Key',
    keyValue: 'sk-9876543210fedcba9876543210fedcba',
    description: '自动化测试专用',
    providerType: 'Google',
    schedulingStrategy: 'weighted',
    providerKeys: ['gemini-test'],
    retryCount: 1,
    timeoutSeconds: 60,
    rateLimitPerMinute: 20,
    tokenLimitPerDay: 5000,
    status: 'active',
    usage: 89,
    limit: 5000,
    createdAt: '2024-01-12',
    lastUsed: '2024-01-16 11:20',
  },
]

/** 弹窗类型 */
type DialogType = 'add' | 'edit' | 'delete' | 'stats' | null

/** 页面主组件 */
const ApiUserKeysPage: React.FC = () => {
  const [data, setData] = useState<ApiKey[]>(initialData)
  const [searchTerm, setSearchTerm] = useState('')
  const [statusFilter, setStatusFilter] = useState<'all' | 'active' | 'disabled'>('all')
  const [selectedItem, setSelectedItem] = useState<ApiKey | null>(null)
  const [dialogType, setDialogType] = useState<DialogType>(null)
  const [showKeyValues, setShowKeyValues] = useState<{ [key: string]: boolean }>({})
  
  // 分页状态
  const [currentPage, setCurrentPage] = useState(1)
  const [pageSize, setPageSize] = useState(10)

  // 过滤数据
  const filteredData = useMemo(() => {
    return data.filter((item) => {
      const matchesSearch = 
        item.keyName.toLowerCase().includes(searchTerm.toLowerCase()) ||
        item.description.toLowerCase().includes(searchTerm.toLowerCase()) ||
        item.providerType.toLowerCase().includes(searchTerm.toLowerCase())
      const matchesStatus = statusFilter === 'all' || item.status === statusFilter
      return matchesSearch && matchesStatus
    })
  }, [data, searchTerm, statusFilter])

  // 分页数据和计算
  const paginatedData = useMemo(() => {
    const startIndex = (currentPage - 1) * pageSize
    return filteredData.slice(startIndex, startIndex + pageSize)
  }, [filteredData, currentPage, pageSize])

  const totalPages = Math.ceil(filteredData.length / pageSize)
  
  // 重置页码当过滤条件改变时
  React.useEffect(() => {
    setCurrentPage(1)
  }, [searchTerm, statusFilter])

  // 生成新的API Key
  const generateApiKey = () => {
    return 'sk-' + Math.random().toString(36).substring(2) + Math.random().toString(36).substring(2)
  }

  // 添加新API Key
  const handleAdd = (newKey: Omit<ApiKey, 'id' | 'usage' | 'createdAt' | 'lastUsed' | 'keyValue'>) => {
    const apiKey: ApiKey = {
      ...newKey,
      id: Date.now().toString(),
      usage: 0,
      createdAt: new Date().toISOString().split('T')[0],
      lastUsed: '从未使用',
      keyValue: generateApiKey(),
    }
    setData([...data, apiKey])
    setDialogType(null)
  }

  // 编辑API Key
  const handleEdit = (updatedKey: ApiKey) => {
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

  // 切换API Key可见性
  const toggleKeyVisibility = (id: string) => {
    setShowKeyValues(prev => ({ ...prev, [id]: !prev[id] }))
  }

  // 复制到剪贴板
  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text)
  }

  // 渲染遮罩的API Key
  const renderMaskedKey = (key: string, id: string) => {
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
      <div className="mb-6 grid grid-cols-1 md:grid-cols-3 gap-4">
        <StatCard
          icon={<Key size={18} />}
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
          icon={<Users size={18} />}
          value={data.reduce((sum, item) => sum + item.usage, 0).toLocaleString()}
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
      <div className="bg-white rounded-2xl border border-neutral-200 overflow-hidden">
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


      {/* 对话框组件将在下一步实现 */}
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
  selectedItem: ApiKey | null
  onClose: () => void
  onAdd: (item: Omit<ApiKey, 'id' | 'usage' | 'createdAt' | 'lastUsed'>) => void
  onEdit: (item: ApiKey) => void
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
    <div className="bg-white rounded-2xl p-6 w-full max-w-2xl mx-4 max-h-[90vh] overflow-y-auto">
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
    <div className="bg-white rounded-2xl p-6 w-full max-w-2xl mx-4 max-h-[90vh] overflow-y-auto">
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
    <div className="bg-white rounded-2xl p-6 w-full max-w-md mx-4">
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
    <div className="bg-white rounded-2xl p-6 w-full max-w-2xl mx-4 max-h-[80vh] overflow-y-auto">
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

export default ApiUserKeysPage

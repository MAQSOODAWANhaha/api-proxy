/**
 * Providers.tsx
 * 服务商 Providers 列表页：
 * - 展示系统支持的服务商类型（启用/禁用）
 * - 支持搜索与状态筛选
 * - UI 风格与现有管理端保持一致
 */

import React, { useCallback, useEffect, useMemo, useState } from 'react'
import {
  Building2,
  CheckCircle2,
  Copy,
  RefreshCw,
  Search,
  XCircle,
} from 'lucide-react'
import { toast } from 'sonner'
import { StatCard } from '../components/common/StatCard'
import FilterSelect from '../components/common/FilterSelect'
import { api, ProviderType } from '../lib/api'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Table,
  TableHeader,
  TableRow,
  TableHead,
  TableBody,
  TableCell,
} from '@/components/ui/table'

type StatusFilter = 'all' | 'active' | 'inactive'

/** 日期格式化（到分钟） */
function formatDate(iso: string) {
  try {
    const d = new Date(iso)
    const y = d.getFullYear()
    const m = String(d.getMonth() + 1).padStart(2, '0')
    const day = String(d.getDate()).padStart(2, '0')
    const hh = String(d.getHours()).padStart(2, '0')
    const mm = String(d.getMinutes()).padStart(2, '0')
    return `${y}-${m}-${day} ${hh}:${mm}`
  } catch {
    return iso
  }
}

/** 认证类型显示文本 */
function authTypeLabel(authType: string) {
  switch (authType) {
    case 'api_key':
      return 'API Key'
    case 'oauth':
    case 'oauth2':
      return 'OAuth'
    default:
      return authType
  }
}

const ProvidersPage: React.FC = () => {
  const [providers, setProviders] = useState<ProviderType[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const [searchTerm, setSearchTerm] = useState('')
  const [statusFilter, setStatusFilter] = useState<StatusFilter>('all')

  /** 拉取服务商类型列表（包含禁用项） */
  const fetchProviders = useCallback(async () => {
    try {
      setLoading(true)
      setError(null)

      const response = await api.auth.getProviderTypes({ include_inactive: true })
      if (response.success && response.data) {
        setProviders(response.data.provider_types || [])
      } else {
        setProviders([])
        setError(response.error?.message || '获取服务商列表失败')
      }
    } catch (e) {
      console.error('[Providers] Failed to fetch provider types:', e)
      setProviders([])
      setError('获取服务商列表异常')
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    fetchProviders()
  }, [fetchProviders])

  /** 统计卡数据 */
  const stats = useMemo(() => {
    const total = providers.length
    const active = providers.filter((p) => p.is_active).length
    const inactive = total - active
    return { total, active, inactive }
  }, [providers])

  /** 本地筛选 */
  const filteredProviders = useMemo(() => {
    const keyword = searchTerm.trim().toLowerCase()
    return providers.filter((p) => {
      const matchesStatus =
        statusFilter === 'all' ||
        (statusFilter === 'active' && p.is_active) ||
        (statusFilter === 'inactive' && !p.is_active)

      const matchesKeyword =
        !keyword ||
        p.name.toLowerCase().includes(keyword) ||
        p.display_name.toLowerCase().includes(keyword) ||
        (p.base_url || '').toLowerCase().includes(keyword)

      return matchesStatus && matchesKeyword
    })
  }, [providers, searchTerm, statusFilter])

  const handleCopy = async (text: string, label: string) => {
    try {
      await navigator.clipboard.writeText(text)
      toast.success(`${label}已复制到剪贴板`)
    } catch {
      toast.error('复制失败，请手动复制')
    }
  }

  return (
    <div className="space-y-4">
      {/* 页面标题与刷新 */}
      <div className="flex items-center justify-between">
        <h1 className="text-lg font-semibold text-neutral-900">服务商 Providers</h1>
        <Button
          variant="outline"
          size="sm"
          onClick={fetchProviders}
          disabled={loading}
        >
          <RefreshCw size={14} className={loading ? 'animate-spin' : ''} />
          <span className="ml-2">刷新</span>
        </Button>
      </div>

      {/* 统计卡 */}
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
        <StatCard
          icon={<Building2 size={18} />}
          value={stats.total.toString()}
          label="服务商总数"
          color="#7c3aed"
        />
        <StatCard
          icon={<CheckCircle2 size={18} />}
          value={stats.active.toString()}
          label="启用服务商"
          color="#10b981"
        />
        <StatCard
          icon={<XCircle size={18} />}
          value={stats.inactive.toString()}
          label="禁用服务商"
          color="#f59e0b"
        />
      </div>

      {/* 搜索与筛选 */}
      <div className="flex items-center gap-4">
        <div className="relative flex-1 max-w-md">
          <Search
            className="absolute left-3 top-1/2 -translate-y-1/2 text-neutral-400"
            size={16}
          />
          <input
            type="text"
            placeholder="搜索服务商名称或URL..."
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            className="w-full rounded-lg border border-neutral-200 py-2 pl-10 pr-4 text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
          />
        </div>

        <FilterSelect
          value={statusFilter}
          onValueChange={(value) => setStatusFilter(value as StatusFilter)}
          options={[
            { value: 'all', label: '全部状态' },
            { value: 'active', label: '启用' },
            { value: 'inactive', label: '禁用' },
          ]}
          placeholder="全部状态"
        />
      </div>

      {/* 数据表格 */}
      <div className="bg-white rounded-2xl border border-neutral-200 overflow-hidden">
        {loading ? (
          <div className="flex items-center justify-center py-12">
            <RefreshCw className="animate-spin text-neutral-400" size={24} />
            <span className="ml-2 text-neutral-600">加载中...</span>
          </div>
        ) : (
          <>
            {error && (
              <div className="px-4 py-3 text-sm text-red-600 bg-red-50 border-b border-red-100">
                {error}
              </div>
            )}

            <div className="overflow-x-auto">
              <Table className="min-w-[980px]">
                <TableHeader className="bg-muted/40">
                  <TableRow>
                    <TableHead className="min-w-[180px]">服务商</TableHead>
                    <TableHead className="min-w-[220px]">Base URL</TableHead>
                    <TableHead className="min-w-[120px]">API 格式</TableHead>
                    <TableHead className="min-w-[160px]">默认模型</TableHead>
                    <TableHead className="min-w-[180px]">认证方式</TableHead>
                    <TableHead className="min-w-[200px]">限制</TableHead>
                    <TableHead className="min-w-[100px]">状态</TableHead>
                    <TableHead className="min-w-[160px]">创建时间</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {filteredProviders.map((p) => (
                    <TableRow key={p.id}>
                      <TableCell className="font-medium text-foreground">
                        <div className="flex flex-col">
                          <span>{p.display_name}</span>
                          <span className="text-xs text-muted-foreground">{p.name}</span>
                        </div>
                      </TableCell>

                      <TableCell>
                        {p.base_url ? (
                          <div className="flex items-center gap-2">
                            <code className="rounded bg-muted px-2 py-0.5 text-xs text-foreground/80">
                              {p.base_url}
                            </code>
                            <Button
                              variant="ghost"
                              size="icon"
                              onClick={() => handleCopy(p.base_url || '', 'Base URL')}
                            >
                              <Copy size={14} className="text-neutral-500" />
                            </Button>
                          </div>
                        ) : (
                          <span className="text-foreground/60">-</span>
                        )}
                      </TableCell>

                      <TableCell>
                        {p.api_format ? (
                          <Badge variant="secondary">{p.api_format}</Badge>
                        ) : (
                          <span className="text-foreground/60">-</span>
                        )}
                      </TableCell>

                      <TableCell className="text-foreground/80">
                        {p.default_model || '-'}
                      </TableCell>

                      <TableCell>
                        <div className="flex flex-wrap gap-1">
                          {(p.supported_auth_types || []).length > 0 ? (
                            p.supported_auth_types.map((t) => (
                              <Badge
                                key={`${p.id}-${t}`}
                                variant="outline"
                                className="text-xs"
                              >
                                {authTypeLabel(t)}
                              </Badge>
                            ))
                          ) : (
                            <span className="text-foreground/60">-</span>
                          )}
                        </div>
                      </TableCell>

                      <TableCell className="text-foreground/70">
                        <div className="flex flex-col gap-0.5 text-xs">
                          <span>MaxTokens: {p.max_tokens ?? '-'}</span>
                          <span>RateLimit: {p.rate_limit ?? '-'} /min</span>
                          <span>Timeout: {p.timeout_seconds ?? '-'}s</span>
                        </div>
                      </TableCell>

                      <TableCell>
                        {p.is_active ? (
                          <Badge
                            variant="outline"
                            className="border-emerald-200 bg-emerald-50 text-emerald-700"
                          >
                            启用
                          </Badge>
                        ) : (
                          <Badge
                            variant="outline"
                            className="border-neutral-200 bg-neutral-50 text-neutral-700"
                          >
                            禁用
                          </Badge>
                        )}
                      </TableCell>

                      <TableCell className="text-foreground/70">
                        {formatDate(p.created_at)}
                      </TableCell>
                    </TableRow>
                  ))}

                  {filteredProviders.length === 0 && (
                    <TableRow>
                      <TableCell colSpan={8} className="py-10 text-center text-neutral-500">
                        暂无匹配的服务商数据
                      </TableCell>
                    </TableRow>
                  )}
                </TableBody>
              </Table>
            </div>
          </>
        )}
      </div>
    </div>
  )
}

export default ProvidersPage


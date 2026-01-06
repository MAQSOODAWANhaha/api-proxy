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
  Pencil,
  Plus,
  RefreshCw,
  Search,
  Trash2,
  XCircle,
} from 'lucide-react'
import { StatCard } from '../components/common/StatCard'
import FilterSelect from '../components/common/FilterSelect'
import { api, ProviderType } from '../lib/api'
import { LoadingSpinner, LoadingState } from '@/components/ui/loading'
import { Skeleton } from '@/components/ui/skeleton'
import { copyWithFeedback } from '../lib/clipboard'
import ProviderTypeDialog from '@/components/provider/ProviderTypeDialog'
import DataTableShell from '@/components/common/DataTableShell'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { toast } from 'sonner'

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

  const [dialogOpen, setDialogOpen] = useState(false)
  const [dialogMode, setDialogMode] = useState<'create' | 'edit'>('create')
  const [editing, setEditing] = useState<ProviderType | null>(null)
  const [openingEdit, setOpeningEdit] = useState(false)

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

  const openCreate = () => {
    setEditing(null)
    setDialogMode('create')
    setDialogOpen(true)
  }

  const openEdit = (p: ProviderType) => {
    void (async () => {
      try {
        setOpeningEdit(true)
        const res = await api.auth.getProviderType(p.id)
        if (!res.success || !res.data) {
          toast.error(res.error?.message || '获取服务商类型详情失败')
          return
        }
        setEditing(res.data.provider_type)
        setDialogMode('edit')
        setDialogOpen(true)
      } finally {
        setOpeningEdit(false)
      }
    })()
  }

  const deleteRow = async (p: ProviderType) => {
    if (!confirm(`确认删除该服务商类型？\\n\\n${p.display_name} (${p.name}) / ${p.auth_type || ''}`)) {
      return
    }
    const res = await api.auth.deleteProviderType(p.id)
    if (!res.success) {
      toast.error(res.error?.message || '删除失败')
      return
    }
    toast.success('删除成功')
    void fetchProviders()
  }

  return (
    <div className="w-full">
      {/* 页面头部 */}
      <div className="flex items-center justify-between mb-6">
        <div>
          <h2 className="text-lg font-medium text-neutral-800">服务商 Providers</h2>
          <p className="text-sm text-neutral-600 mt-1">查看系统内置的AI服务商类型与状态</p>
        </div>
        <div className="flex gap-2">
          <button
            onClick={fetchProviders}
            disabled={loading}
            className="flex items-center gap-2 px-3 py-2 text-sm text-neutral-600 hover:text-neutral-800 disabled:opacity-50 disabled:cursor-not-allowed"
            title="刷新数据"
          >
            {loading ? <LoadingSpinner size="sm" tone="muted" /> : <RefreshCw size={16} />}
            刷新
          </button>
          <button
            onClick={openCreate}
            className="flex items-center gap-2 bg-violet-600 text-white px-4 py-2 rounded-lg text-sm font-medium hover:bg-violet-700"
            title="新增服务商类型"
          >
            <Plus size={16} />
            新增
          </button>
        </div>
      </div>

      {/* 统计卡 */}
      {loading ? (
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-3 mb-6">
          {[1, 2, 3].map((i) => (
            <div
              key={i}
              className="rounded-2xl border border-neutral-200 bg-white p-4"
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
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-3 mb-6">
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
      )}

      {/* 搜索与筛选 */}
      <div className="flex items-center gap-4 mb-4">
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

        <div className="flex items-center gap-4">
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
      </div>

      {/* 数据表格 */}
      <DataTableShell>
        {loading ? (
          <div className="flex items-center justify-center py-12">
            <LoadingState text="加载中..." />
          </div>
        ) : (
          <>
            {error && (
              <div className="px-4 py-3 text-sm text-red-600 bg-red-50 border-b border-red-100">
                {error}
              </div>
            )}

            <Table className="min-w-[780px]">
              <TableHeader>
                <TableRow>
                  <TableHead className="min-w-[180px]">服务商</TableHead>
                  <TableHead className="min-w-[220px]">Base URL</TableHead>
                  <TableHead className="min-w-[180px]">认证方式</TableHead>
                  <TableHead className="min-w-[100px]">状态</TableHead>
                  <TableHead className="min-w-[160px]">创建时间</TableHead>
                  <TableHead className="min-w-[140px] text-right">操作</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {filteredProviders.map((p) => (
                    <TableRow key={p.id}>
                    <TableCell>
                      <div className="flex flex-col">
                        <span className="table-tag">{p.display_name}</span>
                        <span className="table-subtext">{p.name}</span>
                      </div>
                    </TableCell>

                    <TableCell>
                        {p.base_url ? (
                          <div className="flex items-center gap-2">
                            <code className="table-code">
                              {p.base_url}
                            </code>
                            <button
                              onClick={() => void copyWithFeedback(p.base_url || '', 'Base URL')}
                              className="text-neutral-500 hover:text-neutral-700"
                              title="复制 Base URL"
                              aria-label="复制 Base URL"
                            >
                              <Copy size={14} />
                            </button>
                          </div>
                        ) : (
                          <span className="text-foreground/60">-</span>
                        )}
                    </TableCell>

                    <TableCell>
                        <div className="flex flex-wrap gap-1">
                          {p.auth_type ? (
                            <span className="table-tag" key={`${p.id}-${p.auth_type}`}>
                              {authTypeLabel(p.auth_type)}
                            </span>
                          ) : (
                            <span className="text-foreground/60">-</span>
                          )}
                        </div>
                    </TableCell>

                    <TableCell>
                        {p.is_active ? (
                          <span className="table-status-success">启用</span>
                        ) : (
                          <span className="table-status-muted">禁用</span>
                        )}
                    </TableCell>

                    <TableCell className="text-foreground/70">
                        {formatDate(p.created_at)}
                    </TableCell>

                    <TableCell>
                        <div className="flex items-center justify-end gap-1">
                          <button
                            onClick={() => openEdit(p)}
                            disabled={openingEdit}
                            className="p-1 text-neutral-500 hover:text-violet-600"
                            title="编辑"
                          >
                            <Pencil size={14} />
                          </button>
                          <button
                            onClick={() => void deleteRow(p)}
                            className="p-1 text-neutral-500 hover:text-red-600"
                            title="删除"
                          >
                            <Trash2 size={14} />
                          </button>
                        </div>
                    </TableCell>
                  </TableRow>
                ))}

                {filteredProviders.length === 0 && (
                  <TableRow>
                    <TableCell colSpan={6} className="py-10 text-center text-neutral-500">
                      暂无匹配的服务商数据
                    </TableCell>
                  </TableRow>
                )}
              </TableBody>
            </Table>
          </>
        )}
      </DataTableShell>

      <ProviderTypeDialog
        open={dialogOpen}
        mode={dialogMode}
        editing={editing}
        onOpenChange={setDialogOpen}
        onSaved={() => void fetchProviders()}
      />
    </div>
  )
}

export default ProvidersPage

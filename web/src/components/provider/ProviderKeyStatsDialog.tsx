/**
 * ProviderKeyStatsDialog.tsx
 * 账号 API Key 的使用统计弹窗（样式优化 + 按模型统计 + 日维度柱状图）。
 * - 概览数字：近7天请求、近7天Tokens、平均/天
 * - 请求次数（日）与 Token 消耗（日）：使用 Recharts 柱状图（含网格、坐标轴、Tooltip）
 * - 按模型统计：迷你饼图展示近7天请求占比（与 provider 对齐）
 */

import React, { useMemo } from 'react'
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import type { ProviderKeyItem } from './ProviderKeysTable'
import {
  PieChart,
  Pie,
  Cell,
  ResponsiveContainer,
  Tooltip as ReTooltip,
  BarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
} from 'recharts'

/** 组件属性 */
export interface ProviderKeyStatsDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  item?: ProviderKeyItem
}

/**
 * 将字符串哈希为稳定数字（用于生成稳定示例数据）
 */
function hashStr(str: string): number {
  let h = 0
  for (let i = 0; i < str.length; i++) {
    h = (h << 5) - h + str.charCodeAt(i)
    h |= 0
  }
  return Math.abs(h)
}

/**
 * 获取 provider 对应的模型集合
 */
function getModelsByProvider(provider?: string): string[] {
  switch (provider) {
    case 'openai':
      return ['gpt-4o', 'gpt-4', 'gpt-3.5']
    case 'claude':
      return ['claude-3-opus', 'claude-3-sonnet', 'claude-3-haiku']
    case 'gemini':
      return ['gemini-1.5-pro', 'gemini-1.5-flash']
    case 'custom':
      return ['custom-1', 'custom-2', 'custom-3']
    default:
      return ['model-a', 'model-b']
  }
}

/**
 * 生成“按模型统计”数据，使总和与 total 对齐（稳定的伪随机分布）
 */
function makeModelDist(seed: number, provider: string | undefined, total: number) {
  const names = getModelsByProvider(provider)
  const weights = names.map((_, i) => ((seed >> (i + 1)) & 255) + 30)
  const sumW = weights.reduce((s, w) => s + w, 0)
  let remain = total
  const values = weights.map((w, i) => {
    const v = i === names.length - 1 ? remain : Math.round((w / sumW) * total)
    remain -= v
    return v
  })
  const COLORS = ['#6366F1', '#8B5CF6', '#10B981', '#F59E0B', '#0EA5E9']
  return names.map((name, idx) => ({
    name,
    value: Math.max(0, values[idx]),
    color: COLORS[idx % COLORS.length],
  }))
}

/** 数据行（按天） */
interface DayRow {
  /** 完整 ISO 日期（yyyy-mm-dd） */
  iso: string
  /** 用于 X 轴的短标签（MM-DD） */
  label: string
  /** 请求次数 */
  req: number
  /** Token 消耗 */
  tokens: number
}

/**
 * 生成最近 n 天的 ISO 日期字符串列表，含今天，倒序（最旧 -> 最新）
 */
function lastNDays(n: number): string[] {
  const today = new Date()
  const arr: string[] = []
  for (let i = n - 1; i >= 0; i--) {
    const d = new Date(today)
    d.setDate(today.getDate() - i)
    const y = d.getFullYear()
    const m = String(d.getMonth() + 1).padStart(2, '0')
    const day = String(d.getDate()).padStart(2, '0')
    arr.push(`${y}-${m}-${day}`)
  }
  return arr
}

/** 将 ISO 日期转为 MM-DD 展示 */
function md(iso: string): string {
  const [_, m, d] = iso.split('-')
  return `${m}-${d}`
}

/**
 * ProviderKeyStatsDialog
 * - 使用更清晰的分区与对齐，增加按天的柱状图展示
 */
const ProviderKeyStatsDialog: React.FC<ProviderKeyStatsDialogProps> = ({ open, onOpenChange, item }) => {
  /**
   * 生成最近 7 天的请求与 Token 数据
   * 逻辑：以 key id + 每天 ISO 为种子，生产稳定的“演示”数据；具备可读的日标签（MM-DD）
   */
  const days: DayRow[] = useMemo(() => {
    const dates = lastNDays(7)
    const baseId = item?.id || 'seed'
    return dates.map((iso) => {
      const h = hashStr(baseId + iso)
      // 让请求数处于 30~160 之间
      const req = (h % 131) + 30
      // Token 与请求相关联，系数在 8~14 之间
      const tokens = req * (8 + (h % 7))
      return { iso, label: md(iso), req, tokens }
    })
  }, [item?.id])

  const totalReq7d = days.reduce((s, d) => s + d.req, 0)
  const totalTok7d = days.reduce((s, d) => s + d.tokens, 0)
  const avgReq = (totalReq7d / 7).toFixed(1)

  // 近7天按模型分布（请求数占比）
  const modelDist = useMemo(
    () => makeModelDist(hashStr((item?.id || '') + (item?.provider || 'p')), item?.provider, totalReq7d),
    [item?.id, item?.provider, totalReq7d],
  )

  /** Recharts 公共样式：网格线颜色 */
  const gridStroke = '#E5E7EB' // neutral-200

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[820px]">
        <DialogHeader>
          <DialogTitle className="text-base">使用统计 - {item?.name || ''}</DialogTitle>
        </DialogHeader>

        <div className="grid grid-cols-1 gap-4">
          {/* 概览数字（紧凑卡片） */}
          <div className="grid grid-cols-3 gap-3">
            <div className="rounded-lg border bg-white p-3">
              <div className="text-xs text-muted-foreground">近 7 天请求</div>
              <div className="mt-1 text-lg font-semibold tabular-nums">{totalReq7d}</div>
            </div>
            <div className="rounded-lg border bg-white p-3">
              <div className="text-xs text-muted-foreground">近 7 天 Tokens</div>
              <div className="mt-1 text-lg font-semibold tabular-nums">{totalTok7d}</div>
            </div>
            <div className="rounded-lg border bg-white p-3">
              <div className="text-xs text-muted-foreground">平均/天</div>
              <div className="mt-1 text-lg font-semibold tabular-nums">{avgReq}</div>
            </div>
          </div>

          {/* 请求次数（日） */}
          <div className="rounded-lg border bg-white p-3">
            <div className="mb-2 flex items-center justify-between">
              <div className="text-sm font-medium">请求次数（日）</div>
              <div className="text-xs text-muted-foreground">最近 7 天</div>
            </div>
            <div className="h-48 w-full">
              <ResponsiveContainer width="100%" height="100%">
                <BarChart data={days} barSize={26}>
                  <CartesianGrid stroke={gridStroke} vertical={false} />
                  <XAxis
                    dataKey="label"
                    tick={{ fontSize: 11, fill: '#6B7280' }}
                    axisLine={{ stroke: '#D1D5DB' }}
                    tickLine={{ stroke: '#D1D5DB' }}
                    height={32}
                    angle={-20}
                    dx={-4}
                    dy={8}
                  />
                  <YAxis
                    tick={{ fontSize: 11, fill: '#6B7280' }}
                    axisLine={{ stroke: '#D1D5DB' }}
                    tickLine={{ stroke: '#D1D5DB' }}
                    width={36}
                    allowDecimals={false}
                  />
                  <ReTooltip
                    formatter={(value: any) => [`${value}`, '请求数']}
                    labelFormatter={(label: any, payload: any) => {
                      const p = (payload?.[0]?.payload as DayRow) || null
                      return p ? `${p.iso}` : label
                    }}
                    contentStyle={{ fontSize: 12 }}
                  />
                  <Bar dataKey="req" fill="#6366F1" radius={[4, 4, 0, 0]} />
                </BarChart>
              </ResponsiveContainer>
            </div>
          </div>

          {/* Token 消耗（日） */}
          <div className="rounded-lg border bg-white p-3">
            <div className="mb-2 flex items-center justify-between">
              <div className="text-sm font-medium">Token 消耗（日）</div>
              <div className="text-xs text-muted-foreground">最近 7 天</div>
            </div>
            <div className="h-48 w-full">
              <ResponsiveContainer width="100%" height="100%">
                <BarChart data={days} barSize={26}>
                  <CartesianGrid stroke={gridStroke} vertical={false} />
                  <XAxis
                    dataKey="label"
                    tick={{ fontSize: 11, fill: '#6B7280' }}
                    axisLine={{ stroke: '#D1D5DB' }}
                    tickLine={{ stroke: '#D1D5DB' }}
                    height={32}
                    angle={-20}
                    dx={-4}
                    dy={8}
                  />
                  <YAxis
                    tick={{ fontSize: 11, fill: '#6B7280' }}
                    axisLine={{ stroke: '#D1D5DB' }}
                    tickLine={{ stroke: '#D1D5DB' }}
                    width={44}
                  />
                  <ReTooltip
                    formatter={(value: any) => [`${value}`, 'Tokens']}
                    labelFormatter={(label: any, payload: any) => {
                      const p = (payload?.[0]?.payload as DayRow) || null
                      return p ? `${p.iso}` : label
                    }}
                    contentStyle={{ fontSize: 12 }}
                  />
                  <Bar dataKey="tokens" fill="#10B981" radius={[4, 4, 0, 0]} />
                </BarChart>
              </ResponsiveContainer>
            </div>
          </div>

          {/* 按模型统计（饼图） */}
          <div className="rounded-lg border bg-white p-3">
            <div className="mb-2 flex items-center justify-between">
              <div className="text-sm font-medium">按模型统计（请求占比）</div>
              <div className="text-xs text-muted-foreground">近 7 天 · 共 {totalReq7d}</div>
            </div>

            <div className="grid grid-cols-1 items-center gap-3 md:grid-cols-2">
              {/* 饼图 */}
              <div className="h-44">
                <ResponsiveContainer width="100%" height="100%">
                  <PieChart>
                    <ReTooltip formatter={(value: any, name: any) => [`${value}`, `${name}`]} contentStyle={{ fontSize: 12 }} />
                    <Pie
                      data={modelDist}
                      dataKey="value"
                      nameKey="name"
                      innerRadius={50}
                      outerRadius={70}
                      paddingAngle={2}
                      strokeWidth={1}
                    >
                      {modelDist.map((entry, index) => (
                        <Cell key={`cell-${index}`} fill={entry.color} />
                      ))}
                    </Pie>
                  </PieChart>
                </ResponsiveContainer>
              </div>

              {/* 图例 + 进度条 */}
              <div className="space-y-2">
                {modelDist.map((m) => {
                  const pct = totalReq7d ? Math.round((m.value / totalReq7d) * 100) : 0
                  return (
                    <div key={m.name}>
                      <div className="flex items-center justify-between text-sm">
                        <div className="flex items-center gap-2">
                          <span className="inline-block h-2.5 w-2.5 rounded-sm" style={{ backgroundColor: m.color }} aria-hidden />
                          <span className="text-neutral-800">{m.name}</span>
                        </div>
                        <div className="tabular-nums text-neutral-700">{m.value}</div>
                      </div>
                      <div className="mt-1 h-1.5 w-full overflow-hidden rounded bg-neutral-100">
                        <div
                          className="h-full rounded bg-neutral-300"
                          style={{ width: `${pct}%`, backgroundColor: m.color }}
                          aria-label={`${m.name} ${pct}%`}
                        />
                      </div>
                    </div>
                  )
                })}
              </div>
            </div>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}

export default ProviderKeyStatsDialog

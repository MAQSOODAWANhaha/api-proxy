/**
 * Dashboard.tsx
 * 仪表板首页：提供关键指标卡片与简易概览，保证首页不为空白。
 */

import React, { useState, useMemo } from 'react'
import { Activity, Timer, Coins, CheckCircle2, Calendar, ChevronDown, TrendingUp, BarChart, Loader2, AlertCircle, RefreshCw } from 'lucide-react'
import { useDashboardCards } from '../../hooks/useDashboardCards'
import { useModelsRate } from '../../hooks/useModelsRate'
import { useModelsStatistics } from '../../hooks/useModelsStatistics'
import { useTokensTrend } from '../../hooks/useTokensTrend'
import { useUserApiKeysTrend } from '../../hooks/useUserApiKeysTrend'
import { LineChart, Line, XAxis, YAxis, CartesianGrid, Area, AreaChart, PieChart as RechartsPieChart, Pie, Cell } from 'recharts'
import type { TooltipProps } from 'recharts'
import {
  ChartContainer,
  ChartTooltip,
  type ChartConfig,
} from '@/components/ui/chart'

/** 指标项接口 */
interface StatItem {
  key: string
  label: string
  value: string
  delta: string
  icon: React.ReactNode
  color: string
}

/** 时间范围类型 */
type TimeRange = 'today' | '7days' | '30days' | 'custom'

/** 模型使用数据接口 */
interface ModelUsage {
  name: string
  count: number
  percentage: number
  cost: number
  color: string
  successful_requests: number
  failed_requests: number
  success_rate: number
}

/** 自定义日期范围接口 */
interface CustomDateRange {
  startDate: string
  endDate: string
}

/** 趋势图数据点接口 */
interface TrendDataPoint {
  date: string
  requests: number
  tokens: number
}

/** 趋势图显示模式 */
type TrendViewMode = 'requests' | 'tokens'


/** 指标卡片组件 */
const StatCard: React.FC<{ item: StatItem }> = ({ item }) => {
  return (
    <div className="group relative overflow-hidden rounded-2xl border border-neutral-200 bg-white p-4 shadow-sm transition hover:shadow-md">
      {/* 顶部色条 */}
      <div className="absolute inset-x-0 top-0 h-1" style={{ backgroundColor: item.color }} />
      <div className="flex items-center gap-3">
        <div
          className="flex h-10 w-10 items-center justify-center rounded-xl text-white"
          style={{ backgroundColor: item.color }}
          aria-hidden
        >
          {item.icon}
        </div>
        <div className="min-w-0">
          <div className="text-sm text-neutral-500">{item.label}</div>
          <div className="flex items-baseline gap-2">
            <div className="truncate text-xl font-semibold text-neutral-900">{item.value}</div>
            <div className="text-xs text-emerald-600">{item.delta}</div>
          </div>
        </div>
      </div>
    </div>
  )
}


/** 简化的Token趋势图组件 - 使用Recharts */
const SimpleTokenChart: React.FC<{
  data: {
    date: string
    value: number
    prompt_tokens?: number
    completion_tokens?: number
    cache_create?: number
    cache_read?: number
  }[]
}> = ({ data }) => {
  const accentColor = '#6366f1'
  const promptColor = '#38bdf8'
  const completionColor = '#22c55e'
  const cacheCreateColor = '#f97316'
  const cacheReadColor = '#a855f7'

  // 安全地处理数据，过滤无效值
  const chartData = useMemo(() => {
    return data.map((d) => {
      const toNumber = (val: number | undefined) =>
        Number.isFinite(val) ? (val as number) : 0

      return {
        date: d.date,
        value: toNumber(d.value),
        displayDate: new Date(d.date).toLocaleDateString('zh-CN', {
          month: 'short',
          day: 'numeric',
        }),
        prompt_tokens: toNumber(d.prompt_tokens),
        completion_tokens: toNumber(d.completion_tokens),
        cache_create: toNumber(d.cache_create),
        cache_read: toNumber(d.cache_read),
      }
    })
  }, [data])

  const chartConfig = {
    value: {
      label: '总 Token',
      color: accentColor,
    },
    prompt_tokens: {
      label: 'Prompt Token',
      color: promptColor,
    },
    completion_tokens: {
      label: 'Completion Token',
      color: completionColor,
    },
    cache_create: {
      label: '缓存创建',
      color: cacheCreateColor,
    },
    cache_read: {
      label: '缓存读取',
      color: cacheReadColor,
    },
  } satisfies ChartConfig

  const values = chartData.map((d) => d.value)

  // 如果没有数据，显示空状态
  if (chartData.length === 0) {
    return (
      <div className="flex h-80 flex-col items-center justify-center text-neutral-400">
        <div className="text-center">
          <div className="mb-2 text-4xl">📈</div>
          <div className="text-sm">暂无Token趋势数据</div>
        </div>
      </div>
    )
  }

  // 格式化Token数值
  const formatTokenValue = (value: number) => {
    if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`
    if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`
    return value.toLocaleString('zh-CN')
  }

  return (
    <div className="space-y-4">
      {/* 图表 */}
      <div className="h-56">
        <ChartContainer config={chartConfig} className="h-full w-full">
          <AreaChart data={chartData} margin={{ top: 5, right: 20, left: 20, bottom: 5 }}>
            <defs>
              <linearGradient id="tokenAreaGradient" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stopColor="var(--color-value)" stopOpacity={0.35} />
                <stop offset="100%" stopColor="var(--color-value)" stopOpacity={0.05} />
              </linearGradient>
            </defs>
            <CartesianGrid vertical={false} strokeDasharray="4 4" stroke="rgba(99, 102, 241, 0.08)" />
            <XAxis
              dataKey="displayDate"
              tickLine={false}
              axisLine={false}
              tickMargin={8}
            />
            <YAxis
              tickLine={false}
              axisLine={false}
              tickMargin={8}
              tickFormatter={formatTokenValue}
            />
            <ChartTooltip
              cursor={{ stroke: accentColor, strokeOpacity: 0.1, strokeWidth: 1 }}
              content={(tooltipProps: TooltipProps<number, string>) => (
                <TokenTrendTooltip
                  {...tooltipProps}
                  colors={{
                    total: accentColor,
                    prompt: promptColor,
                    completion: completionColor,
                    cacheCreate: cacheCreateColor,
                    cacheRead: cacheReadColor,
                  }}
                />
              )}
            />
            <Area
              type="monotone"
              dataKey="value"
              stroke="var(--color-value)"
              strokeWidth={3}
              fill="url(#tokenAreaGradient)"
              dot={{ strokeWidth: 2, stroke: 'white', r: 4 }}
              activeDot={{ r: 6, strokeWidth: 2, stroke: 'white' }}
            />
            {/* 隐藏的Area用于tooltip显示详细信息 */}
            <Area type="monotone" dataKey="prompt_tokens" stroke="transparent" fill="transparent" hide />
            <Area type="monotone" dataKey="completion_tokens" stroke="transparent" fill="transparent" hide />
            <Area type="monotone" dataKey="cache_create" stroke="transparent" fill="transparent" hide />
            <Area type="monotone" dataKey="cache_read" stroke="transparent" fill="transparent" hide />
          </AreaChart>
        </ChartContainer>
      </div>

      {/* 统计信息 */}
      <div className="grid grid-cols-3 gap-4 border-t border-neutral-100 pt-3">
        <div className="text-center">
          <div className="text-lg font-bold text-neutral-900">
            {formatTokenValue(values[values.length - 1] ?? 0)}
          </div>
          <div className="text-xs text-neutral-500">最新值</div>
        </div>
        <div className="text-center">
          <div className="text-lg font-bold text-neutral-900">
            {formatTokenValue(
              values.length ? Math.round(values.reduce((sum, val) => sum + val, 0) / values.length) : 0
            )}
          </div>
          <div className="text-xs text-neutral-500">平均值</div>
        </div>
        <div className="text-center">
          <div className="text-lg font-bold text-neutral-900">
            {formatTokenValue(values.length ? Math.max(...values) : 0)}
          </div>
          <div className="text-xs text-neutral-500">峰值</div>
        </div>
      </div>
    </div>
  )
}

/** 无控制按钮的趋势图组件 - 使用Recharts */
const TrendChartWithoutControls: React.FC<{
  data: TrendDataPoint[]
  viewMode: TrendViewMode
}> = ({ data, viewMode }) => {
  const chartColor = viewMode === 'requests' ? '#6366f1' : '#0ea5e9'
  const metricLabel = viewMode === 'requests' ? '请求次数' : 'Token数量'
  const gradientId = `user-api-trend-${viewMode}`

  const chartConfig = {
    value: {
      label: metricLabel,
      color: chartColor,
    },
  } satisfies ChartConfig

  // 安全地处理数据，过滤无效值
  const chartData = useMemo(() => {
    return data.map((d, index) => {
      const rawValue = viewMode === 'requests' ? d.requests : d.tokens
      const value = Number.isFinite(rawValue) ? (rawValue as number) : 0
      const prevRaw =
        index > 0
          ? viewMode === 'requests'
            ? data[index - 1].requests
            : data[index - 1].tokens
          : null
      const previousValue =
        prevRaw !== null && Number.isFinite(prevRaw) ? (prevRaw as number) : null

      return {
        date: d.date,
        value,
        displayDate: new Date(d.date).toLocaleDateString('zh-CN', {
          month: 'short',
          day: 'numeric',
        }),
        requests: Number.isFinite(d.requests) ? d.requests : 0,
        tokens: Number.isFinite(d.tokens) ? d.tokens : 0,
        delta: previousValue !== null ? value - previousValue : null,
      }
    })
  }, [data, viewMode])

  const values = chartData.map((d) => d.value)

  // 如果没有数据，显示空状态
  if (chartData.length === 0) {
    return (
      <div className="flex h-80 flex-col items-center justify-center text-neutral-400">
        <div className="text-center">
          <div className="mb-2 text-4xl">📊</div>
          <div className="text-sm">暂无趋势数据</div>
        </div>
      </div>
    )
  }

  const formatValue = (value: number) => Math.round(value).toLocaleString('zh-CN')

  return (
    <div className="space-y-4">
      {/* 图表 */}
      <div className="h-56">
        <ChartContainer config={chartConfig} className="h-full w-full">
          <LineChart data={chartData} margin={{ top: 5, right: 20, left: 20, bottom: 5 }}>
            <defs>
              <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stopColor="var(--color-value)" stopOpacity={0.25} />
                <stop offset="100%" stopColor="var(--color-value)" stopOpacity={0.05} />
              </linearGradient>
            </defs>
            <CartesianGrid vertical={false} strokeDasharray="4 4" stroke="rgba(99, 102, 241, 0.08)" />
            <XAxis
              dataKey="displayDate"
              tickLine={false}
              axisLine={false}
              tickMargin={8}
            />
            <YAxis
              tickLine={false}
              axisLine={false}
              tickMargin={8}
              tickFormatter={formatValue}
            />
            <ChartTooltip
              cursor={{ stroke: chartColor, strokeWidth: 1, strokeOpacity: 0.1 }}
              content={(tooltipProps: TooltipProps<number, string>) => (
                <UserApiTrendTooltip
                  {...tooltipProps}
                  color={chartColor}
                  metricLabel={metricLabel}
                  formatValue={formatValue}
                />
              )}
            />
            <Area type="monotone" dataKey="value" stroke="none" fill={`url(#${gradientId})`} />
            <Line
              type="monotone"
              dataKey="value"
              stroke="var(--color-value)"
              strokeWidth={3}
              dot={{ strokeWidth: 2, stroke: 'white', r: 4 }}
              activeDot={{ r: 6, strokeWidth: 2, stroke: 'white' }}
            />
          </LineChart>
        </ChartContainer>
      </div>

      {/* 统计信息 */}
      <div className="grid grid-cols-3 gap-4 border-t border-neutral-100 pt-3">
        <div className="text-center">
          <div className="text-lg font-bold text-neutral-900">
            {formatValue(values[values.length - 1] ?? 0)}
          </div>
          <div className="text-xs text-neutral-500">{metricLabel}</div>
        </div>
        <div className="text-center">
          <div className="text-lg font-bold text-neutral-900">
            {formatValue(values.length ? values.reduce((sum, val) => sum + val, 0) / values.length : 0)}
          </div>
          <div className="text-xs text-neutral-500">平均值</div>
        </div>
        <div className="text-center">
          <div className="text-lg font-bold text-neutral-900">
            {formatValue(values.length ? Math.max(...values) : 0)}
          </div>
          <div className="text-xs text-neutral-500">峰值</div>
        </div>
      </div>
    </div>
  )
}

/** 紧凑型时间选择器组件 */
const CompactTimeRangeSelector: React.FC<{
  selectedRange: TimeRange
  customRange: CustomDateRange
  onRangeChange: (range: TimeRange) => void
  onCustomRangeChange: (range: CustomDateRange) => void
}> = ({ selectedRange, customRange, onRangeChange, onCustomRangeChange }) => {
  const [showDropdown, setShowDropdown] = useState(false)
  const [showCustomPicker, setShowCustomPicker] = useState(false)

  const timeRangeOptions = [
    { value: 'today' as TimeRange, label: '今天' },
    { value: '7days' as TimeRange, label: '最近7天' },
    { value: '30days' as TimeRange, label: '最近30天' },
    { value: 'custom' as TimeRange, label: '自定义时间' },
  ]

  const getCurrentLabel = () => {
    const option = timeRangeOptions.find(opt => opt.value === selectedRange)
    if (selectedRange === 'custom') {
      return `${customRange.startDate} 至 ${customRange.endDate}`
    }
    return option?.label || '选择时间范围'
  }

  return (
    <div className="relative">
      <button
        onClick={() => setShowDropdown(!showDropdown)}
        className="flex items-center gap-1 rounded-md border border-neutral-200 bg-white px-2 py-1 text-xs hover:bg-neutral-50"
      >
        <Calendar size={12} className="text-neutral-500" />
        <span>{getCurrentLabel()}</span>
        <ChevronDown size={10} className="text-neutral-400" />
      </button>

      {showDropdown && (
        <div className="absolute right-0 z-10 mt-1 w-48 rounded-lg border border-neutral-200 bg-white shadow-lg">
          <div className="p-1">
            {timeRangeOptions.map((option) => (
              <button
                key={option.value}
                onClick={() => {
                  onRangeChange(option.value)
                  if (option.value === 'custom') {
                    setShowCustomPicker(true)
                  } else {
                    setShowCustomPicker(false)
                  }
                  setShowDropdown(false)
                }}
                className={`w-full rounded px-3 py-2 text-left text-sm hover:bg-neutral-50 ${
                  selectedRange === option.value ? 'bg-violet-50 text-violet-700' : 'text-neutral-700'
                }`}
              >
                {option.label}
              </button>
            ))}
          </div>
        </div>
      )}

      {showCustomPicker && selectedRange === 'custom' && (
        <div className="absolute right-0 z-20 mt-1 w-80 rounded-lg border border-neutral-200 bg-white p-4 shadow-lg">
          <div className="space-y-3">
            <div>
              <label className="block text-sm font-medium text-neutral-700 mb-1">开始日期</label>
              <input
                type="date"
                value={customRange.startDate}
                onChange={(e) => onCustomRangeChange({ ...customRange, startDate: e.target.value })}
                className="w-full rounded border border-neutral-200 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-neutral-700 mb-1">结束日期</label>
              <input
                type="date"
                value={customRange.endDate}
                onChange={(e) => onCustomRangeChange({ ...customRange, endDate: e.target.value })}
                className="w-full rounded border border-neutral-200 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
              />
            </div>
            <div className="flex gap-2">
              <button
                onClick={() => setShowCustomPicker(false)}
                className="flex-1 rounded bg-neutral-100 px-3 py-2 text-sm text-neutral-600 hover:bg-neutral-200"
              >
                取消
              </button>
              <button
                onClick={() => setShowCustomPicker(false)}
                className="flex-1 rounded bg-violet-600 px-3 py-2 text-sm text-white hover:bg-violet-700"
              >
                确认
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

/** 饼图 tooltip，按照官方样式扩展显示请求数与成本 */
const ModelUsageTooltip: React.FC<TooltipProps<number, string>> = ({
  active,
  payload,
}) => {
  if (!active || !payload?.length) {
    return null
  }

  const [entry] = payload
  const details = entry.payload as ModelUsage & { percentage: number }

  const rows = [
    {
      label: '请求数',
      value: details.count.toLocaleString(),
      color: details.color,
    },
    {
      label: '成本',
      value: `$${details.cost.toFixed(2)}`,
      color: '#f97316',
    },
    {
      label: '占比',
      value: `${details.percentage.toFixed(1)}%`,
      color: '#22c55e',
    },
  ]

  return (
    <div className="grid min-w-[12rem] items-start gap-2 rounded-lg border border-border/50 bg-background px-3 py-2 text-xs shadow-xl">
      <div className="flex items-center gap-2">
        <span
          className="h-2.5 w-2.5 rounded-sm"
          style={{ backgroundColor: details.color }}
        />
        <span className="font-medium text-foreground">{details.name}</span>
      </div>
      <div className="grid gap-1.5">
        {rows.map((row) => (
          <div key={row.label} className="flex items-center justify-between">
            <span className="text-muted-foreground">{row.label}</span>
            <span className="font-mono font-semibold text-foreground">
              {row.value}
            </span>
          </div>
        ))}
      </div>
    </div>
  )
}

type TokenTrendTooltipProps = TooltipProps<number, string> & {
  colors: {
    total: string
    prompt: string
    completion: string
    cacheCreate: string
    cacheRead: string
  }
}

const TokenTrendTooltip: React.FC<TokenTrendTooltipProps> = ({
  active,
  payload,
  colors,
}) => {
  const point = payload?.[0]?.payload as
    | {
        date: string
        value: number
        prompt_tokens: number
        completion_tokens: number
        cache_create: number
        cache_read: number
      }
    | undefined

  if (!active || !point) {
    return null
  }

  const dateLabel = new Date(point.date).toLocaleDateString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
  })

  const formatNumber = (value: number) =>
    Math.round(value).toLocaleString('zh-CN')

  const rows = [
    { label: '总 Token', value: point.value, color: colors.total, always: true },
    { label: 'Prompt Token', value: point.prompt_tokens, color: colors.prompt },
    {
      label: 'Completion Token',
      value: point.completion_tokens,
      color: colors.completion,
    },
    { label: '缓存创建', value: point.cache_create, color: colors.cacheCreate },
    { label: '缓存读取', value: point.cache_read, color: colors.cacheRead },
  ].filter((row) => row.always || row.value > 0)

  return (
    <div className="grid min-w-[12rem] items-start gap-2 rounded-lg border border-border/50 bg-background px-3 py-2 text-xs shadow-xl">
      <div className="font-medium text-foreground">{dateLabel}</div>
      <div className="grid gap-1.5">
        {rows.map((row) => (
          <div key={row.label} className="flex items-center justify-between gap-3">
            <div className="flex items-center gap-2">
              <span
                className="h-2.5 w-2.5 rounded-sm"
                style={{ backgroundColor: row.color }}
              />
              <span className="text-muted-foreground">{row.label}</span>
            </div>
            <span className="font-mono font-semibold text-foreground">
              {formatNumber(row.value)}
            </span>
          </div>
        ))}
      </div>
    </div>
  )
}

type UserApiTrendTooltipProps = TooltipProps<number, string> & {
  color: string
  metricLabel: string
  formatValue: (value: number) => string
}

const UserApiTrendTooltip: React.FC<UserApiTrendTooltipProps> = ({
  active,
  payload,
  color,
  metricLabel,
  formatValue,
}) => {
  const point = payload?.[0]?.payload as
    | {
        date: string
        value: number
        delta: number | null
      }
    | undefined

  if (!active || !point) {
    return null
  }

  const dateLabel = new Date(point.date).toLocaleDateString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
  })

  const delta = point.delta ?? 0
  const deltaLabel = `${delta >= 0 ? '+' : '-'}${formatValue(Math.abs(delta))}`

  return (
    <div className="grid min-w-[11rem] items-start gap-2 rounded-lg border border-border/50 bg-background px-3 py-2 text-xs shadow-xl">
      <div className="font-medium text-foreground">{dateLabel}</div>
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <span className="h-2.5 w-2.5 rounded-sm" style={{ backgroundColor: color }} />
          <span className="text-muted-foreground">{metricLabel}</span>
        </div>
        <span className="font-mono font-semibold text-foreground">
          {formatValue(point.value)}
        </span>
      </div>
      {point.delta !== null && (
        <div className="flex items-center justify-between gap-2">
          <span className="text-muted-foreground">较前一日</span>
          <span
            className={`font-mono font-semibold ${
              delta >= 0 ? 'text-emerald-500' : 'text-red-500'
            }`}
          >
            {deltaLabel}
          </span>
        </div>
      )}
    </div>
  )
}

/** 饼图组件 - 使用Recharts实现 */
const PieChart: React.FC<{ data: ModelUsage[] }> = ({ data }) => {
  const total = data.reduce((sum, item) => sum + item.count, 0)

  const chartConfig = {
    count: {
      label: '请求数',
      color: 'hsl(var(--chart-1))',
    },
    cost: {
      label: '成本',
      color: '#f97316',
    },
    percentage: {
      label: '占比',
      color: '#22c55e',
    },
  } satisfies ChartConfig

  // 智能处理模型数据显示
  const processedData = useMemo(() => {
    // 按使用量排序
    const sortedData = [...data].sort((a, b) => b.count - a.count)
    
    // 如果模型数量少于等于4个，直接显示全部
    if (sortedData.length <= 4) return sortedData
    
    // 如果有5-6个模型，全部显示
    if (sortedData.length <= 6) return sortedData
    
    // 如果超过6个模型，显示前5个，其余合并为"其他"
    const topModels = sortedData.slice(0, 5)
    const otherModels = sortedData.slice(5)
    
    // 计算"其他"的占比，如果太小（<3%）则合并到前一个模型
    const otherTotal = otherModels.reduce((sum, item) => sum + item.count, 0)
    const otherPercentage = (otherTotal / total) * 100
    
    if (otherPercentage < 3) {
      // 如果"其他"占比太小，显示前6个，不显示"其他"
      return sortedData.slice(0, 6)
    }
    
    const otherCost = otherModels.reduce((sum, item) => sum + item.cost, 0)
    
    return [
      ...topModels,
      {
        name: `其他 (${otherModels.length}个)`,
        count: otherTotal,
        cost: otherCost,
        percentage: otherPercentage,
        color: '#6b7280'
      }
    ]
  }, [data, total])

  // 检查是否有数据
  if (!data.length || total === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-80 text-neutral-400">
        <div className="text-center">
          <div className="text-4xl mb-2">📊</div>
          <div className="text-sm">暂无模型使用数据</div>
        </div>
      </div>
    )
  }

  // 动态调整图例布局：少量模型用单列，多模型用双列
  const legendCols = processedData.length <= 3 ? 1 : 2

  
  return (
    <div className="flex flex-col items-center gap-6">
      {/* Recharts 饼图 */}
      <div className="relative">
        <ChartContainer config={chartConfig} className="h-80 w-80">
          <RechartsPieChart>
            <Pie
              data={processedData}
              cx={160}
              cy={160}
              innerRadius={60}
              outerRadius={120}
              paddingAngle={2}
              dataKey="count"
              stroke="none"
            >
              {processedData.map((entry, index) => (
                <Cell
                  key={`cell-${index}`}
                  fill={entry.color}
                  style={{
                    filter: 'drop-shadow(0 2px 4px rgba(0,0,0,0.1))',
                    cursor: 'pointer'
                  }}
                />
              ))}
            </Pie>
            <ChartTooltip
              cursor={false}
              offset={-40}
              wrapperStyle={{ pointerEvents: 'none' }}
              content={<ModelUsageTooltip />}
            />
          </RechartsPieChart>
        </ChartContainer>
        
        {/* 中心显示总数 */}
        <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
          <div className="text-center">
            <div className="text-3xl font-bold text-neutral-900">{total.toLocaleString()}</div>
            <div className="text-sm text-neutral-500 mt-1">总请求数</div>
          </div>
        </div>
      </div>
      
      {/* 图例 */}
      <div className={`w-full grid gap-2 ${legendCols === 1 ? 'grid-cols-1' : 'grid-cols-2'}`}>
        {processedData.map((item, index) => (
          <div key={index} className="flex items-center gap-2">
            <div
              className="h-3 w-3 rounded-full flex-shrink-0"
              style={{ backgroundColor: item.color }}
            />
            <div className="flex-1 min-w-0">
              <div className="flex items-center justify-between">
                <span 
                  className="text-sm font-medium text-neutral-700 truncate" 
                  title={item.name}
                >
                  {item.name}
                </span>
                <span className="text-sm text-neutral-500 ml-2 flex-shrink-0">
                  {item.percentage.toFixed(1)}%
                </span>
              </div>
            </div>
          </div>
        ))}
      </div>
      
      {/* 如果显示了"其他"分类，添加提示 */}
      {processedData.some(item => item.name.includes('其他')) && (
        <div className="text-xs text-neutral-400 text-center">
          * "其他"包含使用量较少的模型，详情请查看右侧统计列表
        </div>
      )}
    </div>
  )
}

/** 模型统计列表组件 */
const ModelStatsList: React.FC<{ data: ModelUsage[] }> = ({ data }) => {
  const [showAll, setShowAll] = useState(false)
  const sortedData = [...data].sort((a, b) => b.count - a.count)
  
  // 默认显示前5个，可展开查看全部
  const displayData = showAll ? sortedData : sortedData.slice(0, 5)
  const hasMore = sortedData.length > 5

  return (
    <div className="space-y-3">
      <div className="max-h-96 overflow-y-auto space-y-3">
        {displayData.map((item, index) => (
          <div key={index} className="flex items-center justify-between rounded-lg border border-neutral-100 p-3 hover:bg-neutral-50 transition-colors">
            <div className="flex items-center gap-3">
              <div
                className="h-4 w-4 rounded-full flex-shrink-0"
                style={{ backgroundColor: item.color }}
              />
              <div className="min-w-0">
                <div className="font-medium text-neutral-900 truncate">{item.name}</div>
                <div className="text-sm text-neutral-500">{item.count.toLocaleString()} 次请求</div>
              </div>
            </div>
            <div className="text-right flex-shrink-0">
              <div className="font-medium text-neutral-900">${item.cost.toFixed(2)}</div>
              <div className="text-sm text-neutral-500">{item.percentage.toFixed(1)}%</div>
            </div>
          </div>
        ))}
      </div>
      
      {hasMore && (
        <div className="pt-3 mt-2 border-t border-neutral-100">
          <button
            onClick={() => setShowAll(!showAll)}
            className="w-full flex items-center justify-center gap-2 py-3 text-sm text-neutral-600 hover:text-neutral-800 hover:bg-neutral-50 rounded-lg transition-colors"
          >
            <span>{showAll ? '收起' : `查看全部 ${sortedData.length} 个模型`}</span>
            <svg 
              className={`h-4 w-4 transition-transform ${showAll ? 'rotate-180' : ''}`} 
              fill="none" 
              viewBox="0 0 24 24" 
              stroke="currentColor"
            >
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
            </svg>
          </button>
        </div>
      )}
    </div>
  )
}

/**
 * DashboardPage
 * - 4 个指标卡
 * - 欢迎区 + 图表
 */
/** 带独立时间筛选的饼图组件 */
const PieChartWithTimeFilter: React.FC = () => {
  const [selectedTimeRange, setSelectedTimeRange] = useState<TimeRange>('7days')
  const [customDateRange, setCustomDateRange] = useState<CustomDateRange>({
    startDate: new Date(Date.now() - 7 * 24 * 60 * 60 * 1000).toISOString().split('T')[0],
    endDate: new Date().toISOString().split('T')[0]
  })

  // 根据时间范围计算API参数
  const apiParams = useMemo(() => {
    let range = selectedTimeRange
    let start: string | undefined
    let end: string | undefined

    if (selectedTimeRange === 'custom') {
      range = 'custom'
      start = customDateRange.startDate
      end = customDateRange.endDate
    }

    return { range, start, end }
  }, [selectedTimeRange, customDateRange])

  // 使用真实的后端数据
  const { modelsRate, isLoading, error } = useModelsRate(apiParams.range, apiParams.start, apiParams.end)

  // 转换后端数据为组件需要的格式
  const modelData = useMemo(() => {
    if (!modelsRate?.model_usage) return []

    // 为每个模型分配颜色
    const colors = [
      '#7c3aed', '#0ea5e9', '#10b981', '#f59e0b', '#ef4444',
      '#8b5cf6', '#06b6d4', '#84cc16', '#f97316', '#ec4899',
      '#14b8a6', '#a855f7', '#f43f5e', '#22c55e', '#3b82f6'
    ]

    return modelsRate.model_usage.map((item, index) => ({
      name: item.model,
      count: item.usage,
      percentage: (item.usage / modelsRate.model_usage.reduce((sum, m) => sum + m.usage, 0)) * 100,
      cost: item.cost || 0,
      color: colors[index % colors.length],
      successful_requests: item.successful_requests,
      failed_requests: item.failed_requests,
      success_rate: item.success_rate
    }))
  }, [modelsRate])

  return (
    <div className="rounded-2xl border border-neutral-200 bg-white p-6">
      <div className="mb-4 flex items-center justify-between">
        <h3 className="text-sm font-medium text-neutral-900">模型使用占比</h3>
        <CompactTimeRangeSelector
          selectedRange={selectedTimeRange}
          customRange={customDateRange}
          onRangeChange={setSelectedTimeRange}
          onCustomRangeChange={setCustomDateRange}
        />
      </div>
      
      {/* 加载状态 */}
      {isLoading && (
        <div className="flex items-center justify-center h-80">
          <div className="flex items-center gap-2 text-neutral-500">
            <Loader2 className="h-5 w-5 animate-spin" />
            <span className="text-sm">加载模型使用数据...</span>
          </div>
        </div>
      )}

      {/* 错误状态 */}
      {error && !isLoading && (
        <div className="flex items-center justify-center h-80 text-neutral-400">
          <div className="text-center">
            <AlertCircle className="h-8 w-8 mx-auto mb-2 text-red-400" />
            <div className="text-sm text-red-600">{error}</div>
          </div>
        </div>
      )}

      {/* 数据显示 */}
      {!isLoading && !error && <PieChart data={modelData} />}
    </div>
  )
}

/** 带独立时间筛选的统计列表组件 */
const ModelStatsListWithTimeFilter: React.FC = () => {
  const [selectedTimeRange, setSelectedTimeRange] = useState<TimeRange>('7days')
  const [customDateRange, setCustomDateRange] = useState<CustomDateRange>({
    startDate: new Date(Date.now() - 7 * 24 * 60 * 60 * 1000).toISOString().split('T')[0],
    endDate: new Date().toISOString().split('T')[0]
  })

  // 根据时间范围计算API参数
  const apiParams = useMemo(() => {
    let range = selectedTimeRange
    let start: string | undefined
    let end: string | undefined

    if (selectedTimeRange === 'custom') {
      range = 'custom'
      start = customDateRange.startDate
      end = customDateRange.endDate
    }

    return { range, start, end }
  }, [selectedTimeRange, customDateRange])

  // 使用真实的后端数据
  const { modelsStatistics, isLoading, error } = useModelsStatistics(apiParams.range, apiParams.start, apiParams.end)

  // 转换后端数据为组件需要的格式
  const modelData = useMemo(() => {
    if (!modelsStatistics?.model_usage) return []

    // 为每个模型分配颜色
    const colors = [
      '#7c3aed', '#0ea5e9', '#10b981', '#f59e0b', '#ef4444',
      '#8b5cf6', '#06b6d4', '#84cc16', '#f97316', '#ec4899',
      '#14b8a6', '#a855f7', '#f43f5e', '#22c55e', '#3b82f6'
    ]

    return modelsStatistics.model_usage.map((item, index) => ({
      name: item.model,
      count: item.usage,
      percentage: item.percentage,
      cost: item.cost || 0,
      color: colors[index % colors.length],
      // ModelsStatistics接口没有成功失败数据，设置为0
      successful_requests: 0,
      failed_requests: 0,
      success_rate: 0
    }))
  }, [modelsStatistics])

  return (
    <div className="rounded-2xl border border-neutral-200 bg-white p-6">
      <div className="mb-4 flex items-center justify-between">
        <h3 className="text-sm font-medium text-neutral-900">模型使用统计</h3>
        <CompactTimeRangeSelector
          selectedRange={selectedTimeRange}
          customRange={customDateRange}
          onRangeChange={setSelectedTimeRange}
          onCustomRangeChange={setCustomDateRange}
        />
      </div>
      
      {/* 加载状态 */}
      {isLoading && (
        <div className="flex items-center justify-center h-80">
          <div className="flex items-center gap-2 text-neutral-500">
            <Loader2 className="h-5 w-5 animate-spin" />
            <span className="text-sm">加载模型统计数据...</span>
          </div>
        </div>
      )}

      {/* 错误状态 */}
      {error && !isLoading && (
        <div className="flex items-center justify-center h-80 text-neutral-400">
          <div className="text-center">
            <AlertCircle className="h-8 w-8 mx-auto mb-2 text-red-400" />
            <div className="text-sm text-red-600">{error}</div>
          </div>
        </div>
      )}

      {/* 数据显示 */}
      {!isLoading && !error && (
        modelData.length > 0 ? (
          <ModelStatsList data={modelData} />
        ) : (
          <div className="flex flex-col items-center justify-center h-80 text-neutral-400">
            <div className="text-center">
              <div className="text-4xl mb-2">📋</div>
              <div className="text-sm">暂无模型统计数据</div>
            </div>
          </div>
        )
      )}
    </div>
  )
}

/** Token使用趋势图组件 */
const TokenTrendChart: React.FC = () => {
  // 使用真实的后端数据
  const { tokensTrend, isLoading, error } = useTokensTrend()

  // 转换后端数据为组件需要的格式
  const chartData = useMemo(() => {
    if (!tokensTrend?.token_usage) return []

    return tokensTrend.token_usage.map(item => ({
      date: item.timestamp,
      value: item.tokens_prompt + item.tokens_completion + item.cache_create_tokens + item.cache_read_tokens,
      prompt_tokens: item.tokens_prompt,
      completion_tokens: item.tokens_completion,
      cache_create: item.cache_create_tokens,
      cache_read: item.cache_read_tokens
    }))
  }, [tokensTrend])

  return (
    <div className="rounded-2xl border border-neutral-200 bg-white p-6">
      <div className="mb-4">
        <h3 className="text-sm font-medium text-neutral-900">Token使用趋势</h3>
        <p className="text-xs text-neutral-500 mt-1">最近30天Token消耗数量</p>
      </div>
      
      {/* 加载状态 */}
      {isLoading && (
        <div className="flex items-center justify-center h-80">
          <div className="flex items-center gap-2 text-neutral-500">
            <Loader2 className="h-5 w-5 animate-spin" />
            <span className="text-sm">加载Token趋势数据...</span>
          </div>
        </div>
      )}

      {/* 错误状态 */}
      {error && !isLoading && (
        <div className="flex items-center justify-center h-80 text-neutral-400">
          <div className="text-center">
            <AlertCircle className="h-8 w-8 mx-auto mb-2 text-red-400" />
            <div className="text-sm text-red-600">{error}</div>
          </div>
        </div>
      )}

      {/* 数据显示 */}
      {!isLoading && !error && (
        chartData.length > 0 ? (
          <SimpleTokenChart data={chartData} />
        ) : (
          <div className="flex flex-col items-center justify-center h-80 text-neutral-400">
            <div className="text-center">
              <div className="text-4xl mb-2">📈</div>
              <div className="text-sm">暂无Token趋势数据</div>
            </div>
          </div>
        )
      )}
    </div>
  )
}

/** 用户API Keys使用趋势图组件 */
const UserApiKeysTrendChart: React.FC = () => {
  const [viewMode, setViewMode] = useState<TrendViewMode>('requests')

  // 使用真实的后端数据，根据模式切换接口类型
  const { trendData, isLoading, error, currentType, switchTrendType } = useUserApiKeysTrend(
    viewMode === 'requests' ? 'request' : 'token'
  )

  // 当视图模式变化时，切换API接口类型
  const handleViewModeChange = (mode: TrendViewMode) => {
    setViewMode(mode)
    switchTrendType(mode === 'requests' ? 'request' : 'token')
  }

  // 转换后端数据为组件需要的格式
  const chartData = useMemo(() => {
    if (!trendData) return []

    if (currentType === 'request' && 'request_usage' in trendData) {
      return trendData.request_usage.map(item => ({
        date: item.timestamp,
        requests: item.request,
        tokens: 0 // 在请求模式下，tokens设为0
      }))
    } else if (currentType === 'token' && 'token_usage' in trendData) {
      return trendData.token_usage.map(item => ({
        date: item.timestamp,
        requests: 0, // 在token模式下，requests设为0
        tokens: item.total_token
      }))
    }

    return []
  }, [trendData, currentType])

  return (
    <div className="rounded-2xl border border-neutral-200 bg-white p-6">
      <div className="mb-4 flex items-center justify-between">
        <div>
          <h3 className="text-sm font-medium text-neutral-900">用户API Keys使用趋势</h3>
          <p className="text-xs text-neutral-500 mt-1">最近30天数据</p>
        </div>
        
        {/* 切换按钮移动到右上方 */}
        <div className="flex rounded-lg border border-neutral-200 bg-white">
          <button
            onClick={() => handleViewModeChange('requests')}
            className={`flex items-center gap-1 px-3 py-1 text-xs rounded-l-lg transition-colors ${
              viewMode === 'requests' 
                ? 'bg-violet-100 text-violet-700' 
                : 'text-neutral-600 hover:text-neutral-800'
            }`}
          >
            <BarChart size={12} />
            请求次数
          </button>
          <button
            onClick={() => handleViewModeChange('tokens')}
            className={`flex items-center gap-1 px-3 py-1 text-xs rounded-r-lg transition-colors ${
              viewMode === 'tokens' 
                ? 'bg-violet-100 text-violet-700' 
                : 'text-neutral-600 hover:text-neutral-800'
            }`}
          >
            <Coins size={12} />
            Token数量
          </button>
        </div>
      </div>
      
      {/* 加载状态 */}
      {isLoading && (
        <div className="flex items-center justify-center h-80">
          <div className="flex items-center gap-2 text-neutral-500">
            <Loader2 className="h-5 w-5 animate-spin" />
            <span className="text-sm">加载用户API Keys趋势数据...</span>
          </div>
        </div>
      )}

      {/* 错误状态 */}
      {error && !isLoading && (
        <div className="flex items-center justify-center h-80 text-neutral-400">
          <div className="text-center">
            <AlertCircle className="h-8 w-8 mx-auto mb-2 text-red-400" />
            <div className="text-sm text-red-600">{error}</div>
          </div>
        </div>
      )}

      {/* 数据显示 */}
      {!isLoading && !error && (
        chartData.length > 0 ? (
          <TrendChartWithoutControls
            data={chartData}
            viewMode={viewMode}
          />
        ) : (
          <div className="flex flex-col items-center justify-center h-80 text-neutral-400">
            <div className="text-center">
              <div className="text-4xl mb-2">📊</div>
              <div className="text-sm">暂无用户API Keys趋势数据</div>
            </div>
          </div>
        )
      )}
    </div>
  )
}

const DashboardPage: React.FC = () => {
  // 使用自定义hook获取仪表板数据
  const { cards, isLoading, error, refresh } = useDashboardCards()

  // 图标映射
  const iconMap: Record<string, React.ReactNode> = {
    requests: <Activity size={18} />,
    tokens: <Coins size={18} />,
    latency: <Timer size={18} />,
    success: <CheckCircle2 size={18} />
  }

  // 将API数据转换为StatItem格式（保持UI组件不变）
  const stats: StatItem[] = useMemo(() => {
    return cards.map(card => ({
      key: card.key,
      label: card.label,
      value: card.value,
      delta: card.delta,
      icon: iconMap[card.key] || <Activity size={18} />,
      color: card.color
    }))
  }, [cards])

  return (
    <div className="w-full">
      {/* 欢迎区 */}
      <section className="mb-6 rounded-2xl border border-neutral-200 bg-gradient-to-r from-violet-50 to-indigo-50 p-5">
        <h2 className="text-lg font-semibold text-neutral-900">欢迎回来 👋</h2>
        <p className="mt-1 text-sm text-neutral-600">
          这里是系统运行概览与关键指标。更多分析请前往各功能页面。
        </p>
      </section>

      {/* 指标卡片 */}
      <section className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
        {/* 加载状态 */}
        {isLoading && (
          <>
            {[1, 2, 3, 4].map((i) => (
              <div key={i} className="group relative overflow-hidden rounded-2xl border border-neutral-200 bg-white p-4 shadow-sm">
                <div className="flex items-center gap-3">
                  <div className="h-10 w-10 rounded-xl bg-neutral-100 animate-pulse"></div>
                  <div className="min-w-0 flex-1">
                    <div className="h-4 w-16 bg-neutral-100 rounded animate-pulse mb-2"></div>
                    <div className="h-6 w-20 bg-neutral-100 rounded animate-pulse"></div>
                  </div>
                </div>
              </div>
            ))}
          </>
        )}

        {/* 错误状态 */}
        {error && !isLoading && (
          <div className="lg:col-span-4 sm:col-span-2 col-span-1">
            <div className="rounded-2xl border border-red-200 bg-red-50 p-4">
              <div className="flex items-center gap-3">
                <AlertCircle className="h-5 w-5 text-red-600 flex-shrink-0" />
                <div className="flex-1">
                  <h3 className="text-sm font-medium text-red-800">加载仪表板数据失败</h3>
                  <p className="text-sm text-red-600 mt-1">{error}</p>
                </div>
                <button
                  onClick={refresh}
                  className="flex items-center gap-2 px-3 py-1 text-sm text-red-700 border border-red-300 rounded-lg hover:bg-red-100 transition-colors"
                >
                  <RefreshCw className="h-4 w-4" />
                  重试
                </button>
              </div>
            </div>
          </div>
        )}

        {/* 正常数据显示 */}
        {!isLoading && !error && stats.map((s) => (
          <StatCard key={s.key} item={s} />
        ))}

        {/* 有错误但仍显示默认数据 */}
        {error && !isLoading && stats.length > 0 && stats.map((s) => (
          <StatCard key={s.key} item={s} />
        ))}
      </section>

      {/* 模型使用分析 - 2列布局 */}
      <section className="mt-6 grid grid-cols-1 gap-4 lg:grid-cols-2">
        <PieChartWithTimeFilter />
        <ModelStatsListWithTimeFilter />
      </section>

      {/* 趋势分析 - 每个图表独占一行 */}
      <section className="mt-6 space-y-4">
        <TokenTrendChart />
        <UserApiKeysTrendChart />
      </section>
    </div>
  )
}

export default DashboardPage

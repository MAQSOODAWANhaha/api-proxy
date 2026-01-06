/**
 * HealthStatusDetail.tsx
 * 健康状态详情组件
 *
 * 功能：
 * - 解析和显示 health_status_detail JSON 数据
 * - 支持 OpenAI 限流信息的详细展示
 * - 支持错误信息的展示
 */

import React from 'react'
import { Badge } from '@/components/ui/badge'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { AlertCircle, CheckCircle, Clock, Info } from 'lucide-react'
import { useTimezoneStore } from '@/store/timezone'
import { formatUTCtoLocal } from '@/lib/timezone'

/** OpenAI 限流窗口信息 */
interface OpenAILimitWindow {
  used_percent: number
  window_seconds?: number
  resets_at?: number
}

/** OpenAI 限流快照数据 */
interface OpenAIRateLimitSnapshot {
  primary?: OpenAILimitWindow
  secondary?: OpenAILimitWindow
}

/** 429 错误详情 */
interface Error429Detail {
  type: string
  message: string
  plan_type?: string
  resets_in_seconds?: number
}

/** 健康状态详情数据 */
interface HealthStatusDetail {
  data?: OpenAIRateLimitSnapshot & {
    error?: Error429Detail
    [key: string]: any
  }
  updated_at?: string
}

/** 组件属性 */
export interface HealthStatusDetailProps {
  /** 健康状态详情 JSON 字符串 */
  health_status_detail?: string | null
  /** 当前健康状态 */
  health_status: 'healthy' | 'rate_limited' | 'unhealthy'
}

/** 格式化时间戳 */
function formatTimestamp(timestamp?: number, timezone?: string): string {
  if (!timestamp) return '-'
  if (timezone) {
    try {
      const isoString = new Date(timestamp * 1000).toISOString()
      return formatUTCtoLocal(isoString, timezone)
    } catch (error) {
      console.warn('按时区格式化时间戳失败，将回退到本地时间显示:', error)
    }
  }
  try {
    const date = new Date(timestamp * 1000)
    return date.toLocaleString('zh-CN', {
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit'
    })
  } catch {
    return '-'
  }
}

/** 格式化 ISO 时间字符串 */
function formatISOTime(isoString?: string, timezone?: string): string {
  if (!isoString) return '-'
  if (timezone) {
    try {
      return formatUTCtoLocal(isoString, timezone)
    } catch (error) {
      console.warn('按时区格式化 ISO 时间失败，将回退到本地时间显示:', error)
    }
  }
  try {
    const date = new Date(isoString)
    return date.toLocaleString('zh-CN', {
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit'
    })
  } catch {
    return isoString
  }
}

/** 格式化持续时间 */
function formatDuration(seconds?: number): string {
  if (!seconds || seconds <= 0) return '-'

  const days = Math.floor(seconds / 86400)
  const hours = Math.floor((seconds % 86400) / 3600)
  const minutes = Math.floor((seconds % 3600) / 60)
  const secs = seconds % 60

  // 构建时间单位数组，只包含非零值
  const timeUnits = []
  if (days > 0) timeUnits.push(`${days}天`)
  if (hours > 0) timeUnits.push(`${hours}小时`)
  if (minutes > 0) timeUnits.push(`${minutes}分钟`)
  if (secs > 0 || timeUnits.length === 0) timeUnits.push(`${secs}秒`)

  return timeUnits.join('')
}

/** 获取健康状态样式 */
function getHealthStatusClass(status: string): string {
  switch (status) {
    case 'healthy':
      return 'table-status-success'
    case 'rate_limited':
      return 'table-status-warning'
    case 'unhealthy':
      return 'table-status-danger'
    default:
      return 'table-status-muted'
  }
}

/** 根据使用率获取配色 */
function getUsageVisual(percent: number) {
  if (percent >= 85) {
    return {
      barClass: 'bg-red-500',
      textClass: 'text-red-600'
    }
  }
  if (percent >= 60) {
    return {
      barClass: 'bg-yellow-500',
      textClass: 'text-yellow-600'
    }
  }
  return {
    barClass: 'bg-green-500',
    textClass: 'text-green-600'
  }
}

/** 计算剩余秒数 */
function calcRemainingSeconds(resetsAt?: number): number | null {
  if (!resetsAt) return null
  const now = Math.floor(Date.now() / 1000)
  return resetsAt - now
}

/** 格式化 Tooltip 数值 */
function formatTooltipValue(value: unknown): string {
  if (value === null || value === undefined) return '-'
  if (typeof value === 'number') return value.toString()
  if (typeof value === 'string') return value
  try {
    return JSON.stringify(value)
  } catch {
    return String(value)
  }
}

/** 格式化时间显示 */
function formatDateTime(value?: string | number | null, timezone?: string): string | undefined {
  if (value === undefined || value === null || value === '') return undefined
  if (typeof value === 'number') {
    return formatTimestamp(value, timezone)
  }
  return formatISOTime(value, timezone)
}


/** 渲染限流窗口信息 */
const renderLimitWindow = (
  window: OpenAILimitWindow & Record<string, any>,
  title: string
) => {
  if (!window) return null

  // 增强数据验证和容错处理，确保进度条始终显示
  const usagePercent = parseFloat(String(window.used_percent ?? 0)) || 0
  const clampedPercent = Math.max(0, Math.min(usagePercent, 100))
  // 优化时间窗口描述，特别是针对长时间窗口
  let windowDurationText = '未知窗口'
  if (window.window_seconds) {
    const duration = formatDuration(window.window_seconds)
    windowDurationText = duration === '-' ? '未知窗口' : duration
  }
  const remainingSeconds = calcRemainingSeconds(window.resets_at)
  // 改进剩余时间显示逻辑，确保所有情况下都有合理显示
  let remainingLabel = '剩余未知'
  if (remainingSeconds !== null) {
    if (remainingSeconds <= 0) {
      remainingLabel = '已重置'
    } else {
      const formattedDuration = formatDuration(remainingSeconds)
      remainingLabel = formattedDuration === '-' ? '剩余未知' : `剩余 ${formattedDuration}`
    }
  }
  const { barClass, textClass } = getUsageVisual(usagePercent)
  const shouldHighlightRemaining = remainingSeconds !== null && remainingSeconds <= 10
  const remainingClass = shouldHighlightRemaining ? 'text-red-600 font-semibold' : textClass

  const extraEntries = Object.entries(window as Record<string, unknown>).filter(
    ([key]) => !['used_percent', 'window_seconds', 'resets_at'].includes(key)
  )

  return (
    <Tooltip key={title}>
      <TooltipTrigger asChild>
        <div className="cursor-help rounded-md border border-border/60 bg-muted/40 px-3 py-2">
          <div className="grid grid-cols-[auto,1fr,auto] items-center gap-3">
            <Badge
              variant="outline"
              className="border-muted-foreground/30 bg-background px-2 py-0.5 text-[11px] font-medium text-muted-foreground w-14 inline-flex items-center justify-center text-center"
            >
              {windowDurationText}
            </Badge>

            <div className="flex items-center gap-2 min-w-0">
              <div className="h-2 w-20 overflow-hidden rounded-full bg-muted flex-shrink-0">
                <div className={`h-2 ${barClass}`} style={{ width: `${clampedPercent}%` }} />
              </div>
              <span className={`text-xs font-semibold tabular-nums ${textClass} flex-shrink-0`}>
                {usagePercent.toFixed(1)}%
              </span>
            </div>

            <span className={`text-xs tabular-nums ${remainingClass} text-right w-32 justify-self-end whitespace-nowrap overflow-hidden`}>{remainingLabel}</span>
          </div>
        </div>
      </TooltipTrigger>

      <TooltipContent className="max-w-xs space-y-2 rounded-lg border bg-white/95 dark:bg-gray-900/95 border-gray-200 dark:border-gray-700">
        <div className="grid grid-cols-2 gap-x-3 gap-y-1 text-[11px] leading-relaxed">
          <span className="text-muted-foreground">时间窗口</span>
          <span className="text-foreground">{windowDurationText}</span>
          <span className="text-muted-foreground">使用率</span>
          <span className="text-foreground">{usagePercent.toFixed(2)}%</span>
          <span className="text-muted-foreground">重置时间</span>
          <span className="text-foreground">
            {window.resets_at ? formatTimestamp(window.resets_at) : '未知'}
          </span>
          <span className="text-muted-foreground">剩余时间</span>
          <span className="text-foreground whitespace-nowrap">{remainingLabel}</span>
                    {extraEntries.map(([key, value]) => (
            <React.Fragment key={key}>
              <span className="text-muted-foreground">{key}</span>
              <span className="text-foreground">{formatTooltipValue(value)}</span>
            </React.Fragment>
          ))}
        </div>
      </TooltipContent>
    </Tooltip>
  )
}

/** 渲染错误信息 */
const renderErrorInfo = (error: Error429Detail) => {
  return (
    <div className="space-y-3">
      <div className="flex items-start gap-2">
        <AlertCircle className="h-4 w-4 text-red-500 mt-0.5 flex-shrink-0" />
        <div className="flex-1 space-y-2">
          <div>
            <span className="text-sm font-medium">错误类型：</span>
            <Badge variant="destructive" className="ml-2 text-xs">
              {error.type}
            </Badge>
          </div>
          <div>
            <span className="text-sm font-medium">错误消息：</span>
            <span className="text-sm text-muted-foreground ml-2">{error.message}</span>
          </div>
          {error.plan_type && (
            <div>
              <span className="text-sm font-medium">计划类型：</span>
              <span className="text-sm text-muted-foreground ml-2">{error.plan_type}</span>
            </div>
          )}
          {error.resets_in_seconds && (
            <div>
              <span className="text-sm font-medium">重置时间：</span>
              <span className="text-sm text-muted-foreground ml-2">
                {formatDuration(error.resets_in_seconds)}后
              </span>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

/**
 * HealthStatusDetail 组件
 */
const HealthStatusDetail: React.FC<HealthStatusDetailProps> = ({
  health_status_detail,
  health_status
}) => {
  const timezone = useTimezoneStore((state) => state.timezone)
  // 解析健康状态详情数据
  let detailData: HealthStatusDetail | null = null
  if (health_status_detail) {
    try {
      detailData = JSON.parse(health_status_detail)
    } catch {
      // JSON 解析失败，显示原始内容
    }
  }

  if (!detailData) {
    return (
      <div className="flex items-center gap-2">
        <div className={`${getHealthStatusClass(health_status)} gap-1.5`}>
          {health_status === 'healthy' && <CheckCircle className="h-3 w-3" />}
          {health_status === 'rate_limited' && <Clock className="h-3 w-3" />}
          {health_status === 'unhealthy' && <AlertCircle className="h-3 w-3" />}
          {getStatusText(health_status)}
        </div>
      </div>
    )
  }

  const primaryWindow = detailData.data?.primary as (OpenAILimitWindow & Record<string, any>) | undefined
  const secondaryWindow = detailData.data?.secondary as (OpenAILimitWindow & Record<string, any>) | undefined

  // 恢复数据更新时间显示
  const createdAtRaw = detailData.data?.created_at ?? detailData.updated_at
  const createdAtText = formatDateTime(createdAtRaw, timezone)

  const windowSummaries = [
    primaryWindow ? renderLimitWindow(primaryWindow, '') : null,
    secondaryWindow ? renderLimitWindow(secondaryWindow, '') : null
  ].filter(Boolean) as React.ReactNode[]

  const showErrorInfo = Boolean(detailData.data?.error)

  return (
    <TooltipProvider>
      <div className="space-y-2">
        <div className="flex items-center gap-2">
          <div className={`${getHealthStatusClass(health_status)} gap-1.5`}>
            {health_status === 'healthy' && <CheckCircle className="h-3 w-3" />}
            {health_status === 'rate_limited' && <Clock className="h-3 w-3" />}
            {health_status === 'unhealthy' && <AlertCircle className="h-3 w-3" />}
            {getStatusText(health_status)}
          </div>

          {createdAtText && (
            <div className="text-xs text-muted-foreground flex items-center gap-1">
              <Info className="h-3 w-3" />
              更新 {createdAtText}
            </div>
          )}
        </div>

        {windowSummaries.length > 0 ? (
          <div className="space-y-2">{windowSummaries}</div>
        ) : (
          <div className="rounded-md border border-dashed border-muted/60 bg-muted/30 px-3 py-2 text-xs text-muted-foreground">
            暂无法获取限流窗口信息
          </div>
        )}

        {showErrorInfo && detailData.data?.error && (
          <div className="rounded-md border border-red-200 bg-red-50 px-3 py-2">
            {renderErrorInfo(detailData.data.error)}
          </div>
        )}

        {process.env.NODE_ENV === 'development' && (
          <div className="rounded-md border border-dashed border-muted/60 bg-muted/20 px-3 py-2">
            <pre className="text-[10px] leading-4 text-muted-foreground whitespace-pre-wrap break-all">
              {JSON.stringify(detailData, null, 2)}
            </pre>
          </div>
        )}
      </div>
    </TooltipProvider>
  )
}


/** 获取状态文本 */
function getStatusText(status: string): string {
  switch (status) {
    case 'healthy':
      return '健康'
    case 'rate_limited':
      return '限流中'
    case 'unhealthy':
      return '异常'
    default:
      return '未知'
  }
}

export default HealthStatusDetail

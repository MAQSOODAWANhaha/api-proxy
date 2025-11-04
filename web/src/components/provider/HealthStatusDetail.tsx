/**
 * HealthStatusDetail.tsx
 * 健康状态详情组件
 *
 * 功能：
 * - 解析和显示 health_status_detail JSON 数据
 * - 支持 OpenAI 限流信息的详细展示
 * - 支持错误信息的展示
 * - 提供折叠/展开功能
 */

import React, { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { ScrollArea } from '@/components/ui/scroll-area'
import { ChevronDown, ChevronUp, AlertCircle, CheckCircle, Clock, Info } from 'lucide-react'

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
  health_status: 'healthy' | 'warning' | 'error' | 'rate_limited' | 'unhealthy'
}

/** 格式化时间戳 */
function formatTimestamp(timestamp?: number): string {
  if (!timestamp) return '-'
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
function formatISOTime(isoString?: string): string {
  if (!isoString) return '-'
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
  const hours = Math.floor(seconds / 3600)
  const minutes = Math.floor((seconds % 3600) / 60)
  const secs = seconds % 60

  if (days > 0) {
    return `${days}天${hours % 24}小时${minutes}分钟`
  } else if (hours > 0) {
    return `${hours}小时${minutes}分钟`
  } else if (minutes > 0) {
    return `${minutes}分钟${secs}秒`
  } else {
    return `${secs}秒`
  }
}

/** 获取健康状态颜色 */
function getHealthStatusColor(status: string): string {
  switch (status) {
    case 'healthy':
      return 'text-green-600 bg-green-50 border-green-200'
    case 'warning':
      return 'text-yellow-600 bg-yellow-50 border-yellow-200'
    case 'error':
    case 'rate_limited':
    case 'unhealthy':
      return 'text-red-600 bg-red-50 border-red-200'
    default:
      return 'text-gray-600 bg-gray-50 border-gray-200'
  }
}

/** 渲染限流窗口信息 */
const renderLimitWindow = (window: OpenAILimitWindow, title: string) => {
  if (!window) return null

  // 计算剩余时间（如果有重置时间戳）
  const getRemainingTime = () => {
    if (!window.resets_at) return null

    const now = Math.floor(Date.now() / 1000)
    const remaining = window.resets_at - now

    if (remaining <= 0) return '已重置'
    return formatDuration(remaining)
  }

  // 获取状态颜色
  const getUsageColor = (percent: number) => {
    if (percent >= 90) return 'text-red-600 bg-red-50 border-red-200'
    if (percent >= 75) return 'text-yellow-600 bg-yellow-50 border-yellow-200'
    if (percent >= 50) return 'text-blue-600 bg-blue-50 border-blue-200'
    return 'text-green-600 bg-green-50 border-green-200'
  }

  const usageColor = getUsageColor(window.used_percent)
  const remainingTime = getRemainingTime()

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <h4 className="font-medium text-sm text-foreground">{title}</h4>
        <div className={`px-2 py-1 rounded-full text-xs font-medium border ${usageColor}`}>
          {window.used_percent.toFixed(1)}%
        </div>
      </div>

      {/* 进度条 */}
      <div className="space-y-1">
        <div className="flex justify-between text-xs text-muted-foreground">
          <span>使用率</span>
          <span>{window.used_percent.toFixed(1)}%</span>
        </div>
        <div className="w-full bg-gray-200 rounded-full h-2">
          <div
            className={`h-2 rounded-full transition-all ${
              window.used_percent >= 90 ? 'bg-red-500' :
              window.used_percent >= 75 ? 'bg-yellow-500' :
              window.used_percent >= 50 ? 'bg-blue-500' : 'bg-green-500'
            }`}
            style={{ width: `${Math.min(window.used_percent, 100)}%` }}
          />
        </div>
      </div>

      {/* 详细信息网格 */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-3 text-sm">
        <div className="space-y-2">
          <div className="flex justify-between">
            <span className="text-muted-foreground">时间窗口:</span>
            <span className="font-medium">
              {window.window_seconds ? formatDuration(window.window_seconds) : '未知'}
            </span>
          </div>
          <div className="flex justify-between">
            <span className="text-muted-foreground">使用率:</span>
            <span className="font-medium">{window.used_percent.toFixed(2)}%</span>
          </div>
        </div>

        <div className="space-y-2">
          <div className="flex justify-between">
            <span className="text-muted-foreground">重置时间:</span>
            <span className="font-medium">
              {window.resets_at ? formatTimestamp(window.resets_at) : '未知'}
            </span>
          </div>
          <div className="flex justify-between">
            <span className="text-muted-foreground">剩余时间:</span>
            <span className={`font-medium ${remainingTime === '已重置' ? 'text-green-600' : 'text-orange-600'}`}>
              {remainingTime || '计算中...'}
            </span>
          </div>
        </div>
      </div>

      {/* 状态提示 */}
      {window.used_percent >= 90 && (
        <div className="flex items-start gap-2 p-2 bg-red-50 rounded border border-red-200">
          <AlertCircle className="h-4 w-4 text-red-500 mt-0.5 flex-shrink-0" />
          <div className="text-xs text-red-700">
            <strong>警告：</strong>使用率已达到 {window.used_percent.toFixed(1)}%，建议等待窗口重置或降低使用频率。
          </div>
        </div>
      )}

      {window.used_percent >= 75 && window.used_percent < 90 && (
        <div className="flex items-start gap-2 p-2 bg-yellow-50 rounded border border-yellow-200">
          <Info className="h-4 w-4 text-yellow-600 mt-0.5 flex-shrink-0" />
          <div className="text-xs text-yellow-700">
            <strong>注意：</strong>使用率较高({window.used_percent.toFixed(1)}%)，请关注使用情况。
          </div>
        </div>
      )}
    </div>
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
  const [isExpanded, setIsExpanded] = useState(false)

  // 解析健康状态详情数据
  let detailData: HealthStatusDetail | null = null
  if (health_status_detail) {
    try {
      detailData = JSON.parse(health_status_detail)
    } catch {
      // JSON 解析失败，显示原始内容
    }
  }

  // 没有详细信息时显示简化版本
  if (!detailData) {
    return (
      <div className="flex items-center gap-2">
        <div className={`flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium border ${getHealthStatusColor(health_status)}`}>
          {health_status === 'healthy' && <CheckCircle className="h-3 w-3" />}
          {(health_status === 'warning' || health_status === 'error') && <AlertCircle className="h-3 w-3" />}
          {health_status === 'rate_limited' && <Clock className="h-3 w-3" />}
          {getStatusText(health_status)}
        </div>
      </div>
    )
  }

  const hasLimitInfo = detailData.data?.primary || detailData.data?.secondary
  const hasErrorInfo = detailData.data?.error

  // 根据健康状态决定显示内容
  const showLimitInfo = health_status === 'healthy' && hasLimitInfo
  const showErrorInfo = (health_status === 'rate_limited' || health_status === 'error') && hasErrorInfo
  const showDetails = showLimitInfo || showErrorInfo

  return (
    <div className="space-y-3">
      {/* 状态概览 */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <div className={`flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium border ${getHealthStatusColor(health_status)}`}>
            {health_status === 'healthy' && <CheckCircle className="h-3 w-3" />}
            {(health_status === 'warning' || health_status === 'error') && <AlertCircle className="h-3 w-3" />}
            {health_status === 'rate_limited' && <Clock className="h-3 w-3" />}
            {getStatusText(health_status)}
          </div>

          {showDetails && (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setIsExpanded(!isExpanded)}
              className="h-6 px-2 text-xs"
            >
              {isExpanded ? (
                <>
                  <ChevronUp className="h-3 w-3 mr-1" />
                  收起详情
                </>
              ) : (
                <>
                  <ChevronDown className="h-3 w-3 mr-1" />
                  展开详情
                </>
              )}
            </Button>
          )}
        </div>

        {detailData.updated_at && (
          <div className="text-xs text-muted-foreground flex items-center gap-1">
            <Info className="h-3 w-3" />
            更新于 {formatISOTime(detailData.updated_at)}
          </div>
        )}
      </div>

      {/* 详细信息 - 根据状态显示不同内容 */}
      {isExpanded && showDetails && (
        <Card className="border-muted">
          <CardHeader className="pb-3">
            <CardTitle className="text-sm font-medium">
              {health_status === 'healthy' ? '限流窗口信息' : '限流错误详情'}
            </CardTitle>
          </CardHeader>
          <CardContent className="pt-0">
            <ScrollArea className="max-h-96">
              <div className="space-y-4">
                {/* 429 错误信息 - 仅在限流/错误状态显示 */}
                {showErrorInfo && detailData.data?.error && (
                  <div className="space-y-2">
                    <h3 className="text-sm font-medium text-red-600">429 限流错误</h3>
                    {renderErrorInfo(detailData.data.error)}
                  </div>
                )}

                {/* 限流窗口信息 - 仅在健康状态显示 */}
                {showLimitInfo && (
                  <div className="space-y-4">

                    {/* 主要限流窗口 */}
                    {detailData.data?.primary && (
                      <div className="border rounded-lg p-4 bg-blue-50/50">
                        {renderLimitWindow(detailData.data.primary, '主要限制窗口')}
                      </div>
                    )}

                    {/* 次要限流窗口 */}
                    {detailData.data?.secondary && (
                      <div className="border rounded-lg p-4 bg-green-50/50">
                        {renderLimitWindow(detailData.data.secondary, '次要限制窗口')}
                      </div>
                    )}
                  </div>
                )}

                {/* 原始数据（调试用） */}
                {process.env.NODE_ENV === 'development' && (
                  <div className="space-y-2">
                    <h3 className="text-sm font-medium text-muted-foreground">原始数据</h3>
                    <pre className="text-xs bg-muted p-2 rounded overflow-auto">
                      {JSON.stringify(detailData, null, 2)}
                    </pre>
                  </div>
                )}
              </div>
            </ScrollArea>
          </CardContent>
        </Card>
      )}
    </div>
  )
}

/** 获取状态文本 */
function getStatusText(status: string): string {
  switch (status) {
    case 'healthy':
      return '健康'
    case 'warning':
      return '警告'
    case 'error':
      return '错误'
    case 'rate_limited':
      return '限流中'
    case 'unhealthy':
      return '异常'
    default:
      return '未知'
  }
}

export default HealthStatusDetail

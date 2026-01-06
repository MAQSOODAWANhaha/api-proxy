import React, { useEffect, useMemo, useState } from 'react'
import { LineChart, Line, CartesianGrid, XAxis, YAxis } from 'recharts'
import { api } from '../../../../lib/api'
import { createSafeStats, safeCurrency, safeLargeNumber, safePercentage, safeResponseTime } from '../../../../lib/dataValidation'
import {
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from '@/components/ui/chart'
import { LocalProviderKey, ProviderKeyTrendPoint } from '../types'
import { LoadingSpinner } from '../../../../components/ui/loading'

/** 统计对话框 */
const StatsDialog: React.FC<{
  item: LocalProviderKey
  onClose: () => void
}> = ({ item, onClose }) => {
  // 使用数据验证工具创建安全的统计数据
  const usageStats = createSafeStats(item.usage)

  // 趋势数据状态管理
  const [trendSeries, setTrendSeries] = useState<ProviderKeyTrendPoint[]>([])
  const [trendLoading, setTrendLoading] = useState(true)

  // 获取趋势数据
  useEffect(() => {
    const fetchTrendData = async () => {
      try {
        setTrendLoading(true)
        const response = await api.providerKeys.getTrends(item.id.toString(), { days: 7 })
        if (response.success && response.data && Array.isArray(response.data.trend_data)) {
          const formatted = response.data.trend_data.map((point: any) => ({
            date: typeof point?.date === 'string' ? point.date : '',
            requests: Number(point?.requests ?? 0),
            cost: Number(point?.cost ?? 0),
          })) as ProviderKeyTrendPoint[]

          const withSortedDates = formatted.some((p) => p.date)
            ? [...formatted].sort((a, b) => {
                const aTime = new Date(a.date).getTime()
                const bTime = new Date(b.date).getTime()
                if (Number.isNaN(aTime) || Number.isNaN(bTime)) {
                  return 0
                }
                return aTime - bTime
              })
            : formatted

          setTrendSeries(withSortedDates)
        } else {
          setTrendSeries([])
        }
      } catch (error) {
        console.error('获取趋势数据失败:', error)
        setTrendSeries([])
      } finally {
        setTrendLoading(false)
      }
    }

    fetchTrendData()
  }, [item.id])

  const stats = {
    ...usageStats,
  }

  const chartSeries = trendSeries

  const formatDateLabel = (value: string) => {
    const parsed = new Date(value)
    if (Number.isNaN(parsed.getTime())) {
      return value
    }
    return `${parsed.getMonth() + 1}/${parsed.getDate()}`
  }

  const trendChartData = useMemo(
    () =>
      chartSeries.map((point, index) => ({
        ...point,
        label: formatDateLabel(point.date) || `Day ${index + 1}`,
      })),
    [chartSeries]
  )

  const successRateDisplay = useMemo(
    () => safePercentage(stats.successRate).toFixed(2),
    [stats.successRate]
  )

  const chartConfig = {
    requests: {
      label: '请求数',
      color: 'hsl(var(--chart-1))',
    },
    cost: {
      label: '花费',
      color: 'hsl(var(--chart-2))',
    },
  } satisfies ChartConfig

  return (
    <div className="bg-white rounded-2xl p-6 w-full max-w-3xl mx-4 max-h-[80vh] overflow-y-auto border border-neutral-200">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-lg font-medium text-neutral-900">账号密钥统计</h3>
        <button onClick={onClose} className="text-neutral-500 hover:text-neutral-700">
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
            <div className="text-2xl font-bold text-violet-900">{safeLargeNumber(stats.totalRequests)}</div>
          </div>
          <div className="p-4 bg-orange-50 rounded-xl">
            <div className="text-sm text-orange-600">总花费</div>
            <div className="text-2xl font-bold text-orange-900">{safeCurrency(stats.totalCost)}</div>
          </div>
          <div className="p-4 bg-emerald-50 rounded-xl">
            <div className="text-sm text-emerald-600">成功率</div>
            <div className="text-2xl font-bold text-emerald-900">{successRateDisplay}%</div>
          </div>
          <div className="p-4 bg-blue-50 rounded-xl">
            <div className="text-sm text-blue-600">平均响应时间</div>
            <div className="text-2xl font-bold text-blue-900">{safeResponseTime(stats.avgResponseTime)}</div>
          </div>
        </div>

        {/* 使用与花费趋势 */}
        <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
          <div>
            <h4 className="text-sm font-medium text-neutral-900 mb-3">7天使用趋势</h4>
            <div className="h-40 w-full">
              {trendLoading ? (
                <div className="flex h-full items-center justify-center text-neutral-500">
                  <LoadingSpinner size="md" tone="primary" />
                </div>
              ) : chartSeries.length === 0 ? (
                <div className="flex h-full items-center justify-center text-neutral-400 text-sm">
                  暂无趋势数据
                </div>
              ) : (
                <ChartContainer config={chartConfig} className="w-full h-full">
                  <LineChart data={trendChartData}>
                    <CartesianGrid vertical={false} />
                    <XAxis dataKey="label" tickLine={false} axisLine={false} tickMargin={8} />
                    <YAxis tickLine={false} axisLine={false} tickMargin={8} />
                    <ChartTooltip cursor={false} content={<ChartTooltipContent indicator="dot" />} />
                    <Line
                      type="monotone"
                      dataKey="requests"
                      stroke="var(--color-requests)"
                      strokeWidth={2}
                      dot={{ r: 3, strokeWidth: 2 }}
                      activeDot={{ r: 5 }}
                    />
                  </LineChart>
                </ChartContainer>
              )}
            </div>
          </div>

          <div>
            <h4 className="text-sm font-medium text-neutral-900 mb-3">7天花费趋势</h4>
            <div className="h-40 w-full">
              {trendLoading ? (
                <div className="flex h-full items-center justify-center text-neutral-500">
                  <LoadingSpinner size="md" tone="primary" />
                </div>
              ) : chartSeries.length === 0 ? (
                <div className="flex h-full items-center justify-center text-neutral-400 text-sm">
                  暂无趋势数据
                </div>
              ) : (
                <ChartContainer config={chartConfig} className="w-full h-full">
                  <LineChart data={trendChartData}>
                    <CartesianGrid vertical={false} />
                    <XAxis dataKey="label" tickLine={false} axisLine={false} tickMargin={8} />
                    <YAxis
                      tickFormatter={(value: number) => `$${Number(value).toFixed(2)}`}
                      tickLine={false}
                      axisLine={false}
                      tickMargin={8}
                    />
                    <ChartTooltip cursor={false} content={<ChartTooltipContent indicator="dot" />} />
                    <Line
                      type="monotone"
                      dataKey="cost"
                      stroke="var(--color-cost)"
                      strokeWidth={2}
                      dot={{ r: 3, strokeWidth: 2 }}
                      activeDot={{ r: 5 }}
                    />
                  </LineChart>
                </ChartContainer>
              )}
            </div>
          </div>
        </div>

        {/* 详细统计 */}
        <div className="grid grid-cols-3 gap-4">
          <div className="p-4 bg-neutral-50 rounded-xl">
            <div className="text-sm text-neutral-600">总Token数</div>
            <div className="text-2xl font-bold text-neutral-900">{safeLargeNumber(stats.totalTokens)}</div>
          </div>
          <div className="p-4 bg-neutral-50 rounded-xl">
            <div className="text-sm text-neutral-600">成功请求数</div>
            <div className="text-2xl font-bold text-emerald-900">{safeLargeNumber(stats.successfulRequests)}</div>
          </div>
          <div className="p-4 bg-neutral-50 rounded-xl">
            <div className="text-sm text-neutral-600">失败请求数</div>
            <div className="text-2xl font-bold text-red-900">{safeLargeNumber(stats.failedRequests)}</div>
          </div>
        </div>

        {/* 限制配置 */}
        <div>
          <h4 className="text-sm font-medium text-neutral-900 mb-3">限制配置</h4>
          <div className="grid grid-cols-3 gap-4">
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">速率限制/分钟</div>
              <div className="font-medium">
                {item.requestLimitPerMinute
                  ? `${item.requestLimitPerMinute.toLocaleString()} 次/分钟`
                  : '无'}
              </div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">Token限制/分钟</div>
              <div className="font-medium">
                {item.tokenLimitPromptPerMinute
                  ? `${item.tokenLimitPromptPerMinute.toLocaleString()} Token/分钟`
                  : '无'}
              </div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">速率限制/天</div>
              <div className="font-medium">
                {item.requestLimitPerDay ? `${item.requestLimitPerDay.toLocaleString()} 次/天` : '无'}
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}

export default StatsDialog

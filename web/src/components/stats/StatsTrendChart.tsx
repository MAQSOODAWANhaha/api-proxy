import { useMemo } from 'react'
import { Area, AreaChart, CartesianGrid, XAxis } from 'recharts'

import {
  ChartConfig,
  ChartContainer,
  ChartLegend,
  ChartLegendContent,
  ChartTooltip,
  ChartTooltipContent,
} from '@/components/ui/chart'
import type { TrendPoint } from '@/types/stats'
import { useTimezoneStore } from '@/store/timezone'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import type { Timeframe } from '@/store/stats'
import { LoadingSpinner } from '@/components/ui/loading'
import { useIsMobile } from '@/hooks/use-mobile'

interface StatsTrendChartProps {
  data: TrendPoint[]
  loading?: boolean
  hasFetched: boolean
  timeframe: Timeframe
  onTimeframeChange?: (value: Timeframe) => void
  timeframeOptions?: Timeframe[]
}

const chartConfig: ChartConfig = {
  requests: {
    label: '请求次数',
    color: 'hsl(var(--chart-1))',
  },
  tokens: {
    label: 'Token 消耗',
    color: 'hsl(var(--chart-2))',
  },
  cost: {
    label: '费用',
    color: 'hsl(var(--chart-3))',
  },
}

export function StatsTrendChart({
  data,
  loading,
  hasFetched,
  timeframe,
  onTimeframeChange,
  timeframeOptions = ['7d', '30d'] as Timeframe[],
}: StatsTrendChartProps) {
  const timezone = useTimezoneStore((state) => state.timezone)
  const isMobile = useIsMobile()

  const chartData = useMemo(() => {
    return data
      .map((item) => ({
        ...item,
        requests: item.requests ?? 0,
        tokens: item.tokens ?? 0,
        cost: item.cost ?? 0,
        success_rate: item.success_rate ?? 0,
        label: new Date(item.timestamp).toLocaleDateString('zh-CN', {
          timeZone: timezone,
          month: '2-digit',
          day: '2-digit',
        }),
      }))
  }, [data, timezone])

  return (
    <Card className="rounded-2xl border border-neutral-200 bg-white">
      <CardHeader className="pb-4">
        <div className="flex w-full flex-col gap-3">
          <div className="flex flex-wrap items-center justify-between gap-4">
            <CardTitle className="text-lg font-semibold text-neutral-800">请求趋势</CardTitle>
            <ToggleGroup
              type="single"
              value={timeframe}
              onValueChange={(value) => {
                if (!value) return
                onTimeframeChange?.(value as Timeframe)
              }}
              className="rounded-lg border border-neutral-200 bg-neutral-50 p-1 text-xs"
            >
              {timeframeOptions.map((option) => (
                <ToggleGroupItem
                  key={option}
                  value={option}
                  className="px-3 py-1 text-neutral-700 data-[state=on]:bg-white data-[state=on]:text-violet-700"
                >
                  {option === '7d' ? '近 7 天' : option === '30d' ? '近 30 天' : '近 90 天'}
                </ToggleGroupItem>
              ))}
            </ToggleGroup>
          </div>
          <CardDescription className="leading-relaxed">
            请求次数、Token 消耗、费用趋势（{timezone} 时区）
          </CardDescription>
        </div>
      </CardHeader>
      <CardContent className="pt-0">
        {loading ? (
          <div className="flex h-[280px] items-center justify-center text-muted-foreground">
            <LoadingSpinner size="md" tone="muted" />
          </div>
        ) : !hasFetched ? (
          <div className="flex h-[280px] items-center justify-center text-sm text-muted-foreground">
            输入查询条件后，将在此展示趋势图。
          </div>
        ) : chartData.length === 0 ? (
          <div className="flex h-[280px] items-center justify-center text-sm text-muted-foreground">
            当前条件下暂无趋势数据。
          </div>
        ) : (
          <ChartContainer config={chartConfig} className="aspect-auto h-[240px] w-full sm:h-[280px]">
            <AreaChart data={chartData} margin={{ left: 0, right: 8, top: 8, bottom: 0 }}>
              <defs>
                <linearGradient id="fillRequests" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor="var(--color-requests)" stopOpacity={0.35} />
                  <stop offset="95%" stopColor="var(--color-requests)" stopOpacity={0.05} />
                </linearGradient>
                <linearGradient id="fillTokens" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor="var(--color-tokens)" stopOpacity={0.35} />
                  <stop offset="95%" stopColor="var(--color-tokens)" stopOpacity={0.05} />
                </linearGradient>
                <linearGradient id="fillCost" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor="var(--color-cost)" stopOpacity={0.35} />
                  <stop offset="95%" stopColor="var(--color-cost)" stopOpacity={0.05} />
                </linearGradient>
              </defs>
              <CartesianGrid vertical={false} strokeDasharray="4 4" stroke="rgba(148,163,184,0.2)" />
              <XAxis
                dataKey="label"
                tickLine={false}
                axisLine={false}
                tickMargin={isMobile ? 8 : 12}
                minTickGap={isMobile ? 28 : 20}
                interval="preserveStartEnd"
              />
              <ChartTooltip
                cursor={false}
                content={<ChartTooltipContent indicator="dot" labelFormatter={(value) => value as string} />}
              />
              <ChartLegend
                content={<ChartLegendContent className="flex-wrap justify-start gap-3 sm:justify-center" />}
              />
              <Area
                dataKey="requests"
                type="monotone"
                stroke="var(--color-requests)"
                fill="url(#fillRequests)"
                strokeWidth={2}
              />
              <Area
                dataKey="tokens"
                type="monotone"
                stroke="var(--color-tokens)"
                fill="url(#fillTokens)"
                strokeWidth={2}
              />
              <Area
                dataKey="cost"
                type="monotone"
                stroke="var(--color-cost)"
                fill="url(#fillCost)"
                strokeWidth={2}
              />
            </AreaChart>
          </ChartContainer>
        )}
      </CardContent>
    </Card>
  )
}

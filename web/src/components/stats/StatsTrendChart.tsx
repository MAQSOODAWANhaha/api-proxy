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
import { useStatsStore } from '@/store/stats'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Loader2 } from 'lucide-react'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'

interface StatsTrendChartProps {
  data: TrendPoint[]
  loading?: boolean
  hasFetched: boolean
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

const timeframeDaysMap = {
  '90d': 90,
  '30d': 30,
  '7d': 7,
} as const

export function StatsTrendChart({ data, loading, hasFetched }: StatsTrendChartProps) {
  const timezone = useTimezoneStore((state) => state.timezone)
  const { filters, setTimeframe } = useStatsStore()
  const timeframe = filters.timeframe

  const chartData = useMemo(() => {
    const windowDays = timeframeDaysMap[timeframe]
    const now = new Date()

    return data
      .filter((item) => {
        const ts = new Date(item.timestamp)
        const diff = (now.getTime() - ts.getTime()) / (1000 * 60 * 60 * 24)
        return diff <= windowDays
      })
      .map((item) => ({
        ...item,
        label: new Date(item.timestamp).toLocaleDateString('zh-CN', {
          timeZone: timezone,
          month: '2-digit',
          day: '2-digit',
        }),
      }))
  }, [data, timeframe, timezone])

  return (
    <Card className="shadow-sm">
      <CardHeader className="pb-4">
        <div className="flex w-full flex-col gap-3">
          <div className="flex flex-wrap items-center justify-between gap-4">
            <CardTitle className="text-lg font-semibold">请求趋势</CardTitle>
            <Select value={timeframe} onValueChange={(value) => setTimeframe(value as typeof timeframe)}>
              <SelectTrigger className="w-[140px]">
                <SelectValue placeholder="近 90 天" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="90d">近 90 天</SelectItem>
                <SelectItem value="30d">近 30 天</SelectItem>
                <SelectItem value="7d">近 7 天</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <CardDescription className="leading-relaxed">
            请求次数、Token 消耗、费用趋势（{timezone} 时区）
          </CardDescription>
        </div>
      </CardHeader>
      <CardContent className="pt-0">
        {loading ? (
          <div className="flex h-[280px] items-center justify-center text-muted-foreground">
            <Loader2 className="h-5 w-5 animate-spin" />
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
          <ChartContainer config={chartConfig} className="h-[280px] w-full">
            <AreaChart data={chartData}>
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
                tickMargin={12}
                minTickGap={20}
              />
              <ChartTooltip
                cursor={false}
                content={<ChartTooltipContent indicator="dot" labelFormatter={(value) => value as string} />}
              />
              <ChartLegend content={<ChartLegendContent />} />
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

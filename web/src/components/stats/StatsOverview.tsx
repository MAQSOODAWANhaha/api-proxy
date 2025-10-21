import { type ReactNode } from 'react'
import { Loader2, BarChart4, Coins, DollarSign } from 'lucide-react'

import { SummaryMetric } from '@/types/stats'
import { Card, CardContent } from '@/components/ui/card'

interface StatsOverviewProps {
  metrics: SummaryMetric[]
  loading?: boolean
  hasFetched: boolean
}

const formatValue = (value: number, unit: SummaryMetric['unit']) => {
  switch (unit) {
    case 'usd':
      return `$${value.toFixed(2)}`
    case 'token':
      if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`
      if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`
      return value.toLocaleString()
    default:
      return value.toLocaleString()
  }
}

const iconMap: Record<string, ReactNode> = {
  BarChart4: <BarChart4 className="h-5 w-5" />,
  Coins: <Coins className="h-5 w-5" />,
  DollarSign: <DollarSign className="h-5 w-5" />,
}

const colorMap: Record<string, { bg: string; fg: string }> = {
  requests: { bg: 'bg-indigo-50', fg: 'text-indigo-600' },
  tokens: { bg: 'bg-sky-50', fg: 'text-sky-600' },
  cost: { bg: 'bg-amber-50', fg: 'text-amber-600' },
}

export function StatsOverview({ metrics, loading, hasFetched }: StatsOverviewProps) {
  if (loading && !hasFetched) {
    return (
      <div className="grid gap-4 md:grid-cols-3">
        {[1, 2, 3].map((key) => (
          <Card key={key} className="border border-dashed">
            <CardContent className="flex h-32 items-center justify-center text-muted-foreground">
              <Loader2 className="h-5 w-5 animate-spin" />
            </CardContent>
          </Card>
        ))}
      </div>
    )
  }

  if (!hasFetched) {
    return (
      <Card className="border border-dashed">
        <CardContent className="flex h-32 flex-col items-center justify-center text-center text-muted-foreground">
          <p className="text-sm">请输入有效的用户 API Key 并点击“查询统计”加载数据。</p>
        </CardContent>
      </Card>
    )
  }

  return (
    <div className="grid gap-4 md:grid-cols-3">
      {metrics.map((metric) => {
        const deltaPositive = metric.delta >= 0
        const magnitude = Math.abs(metric.delta).toFixed(1)
        const icon = iconMap[metric.icon] ?? <BarChart4 className="h-5 w-5" />
        const color = colorMap[metric.id] ?? { bg: 'bg-neutral-100', fg: 'text-neutral-600' }

        return (
          <Card key={metric.id} className="border border-neutral-200 shadow-sm">
            <CardContent className="flex h-full flex-col gap-4 p-5">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <span
                    className={`flex h-10 w-10 items-center justify-center rounded-xl ${color.bg} ${color.fg}`}
                  >
                    {icon}
                  </span>
                  <div className="flex flex-col">
                    <span className="text-sm font-medium text-neutral-600">{metric.label}</span>
                    <span className={`text-xs ${deltaPositive ? 'text-emerald-600' : 'text-rose-600'}`}>
                      环比 {deltaPositive ? '+' : '-'}{magnitude}%
                    </span>
                  </div>
                </div>
              </div>

              <div className="space-y-2">
                <div className="flex items-end justify-between">
                  <span className="text-xs text-neutral-500">今日</span>
                  <span className="text-2xl font-semibold text-neutral-900">
                    {formatValue(metric.today, metric.unit)}
                  </span>
                </div>
                <div className="flex items-center justify-between rounded-lg bg-neutral-50 px-3 py-2 text-xs text-neutral-500">
                  <span>总计</span>
                  <span className="font-medium text-neutral-700">
                    {formatValue(metric.total, metric.unit)}
                  </span>
                </div>
              </div>
            </CardContent>
          </Card>
        )
      })}
    </div>
  )
}

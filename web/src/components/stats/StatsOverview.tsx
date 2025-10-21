import { Loader2, BarChart4, Coins, DollarSign } from 'lucide-react'

import { SummaryMetric } from '@/types/stats'
import { Card, CardContent } from '@/components/ui/card'
import { StatCard } from '@/components/common/StatCard'

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

const iconMap: Record<string, JSX.Element> = {
  BarChart4: <BarChart4 className="h-5 w-5" />,
  Coins: <Coins className="h-5 w-5" />,
  DollarSign: <DollarSign className="h-5 w-5" />,
}

const colorMap: Record<string, string> = {
  requests: '#6366f1',
  tokens: '#0ea5e9',
  cost: '#f97316',
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
        const cardColor = colorMap[metric.id] ?? '#3b82f6'
        const icon = iconMap[metric.icon] ?? <BarChart4 className="h-5 w-5" />

        return (
          <StatCard
            key={metric.id}
            icon={icon}
            color={cardColor}
            label={metric.label}
            value={formatValue(metric.today, metric.unit)}
            delta={`${deltaPositive ? '+' : '-'}${magnitude}%`}
          />
        )
      })}
    </div>
  )
}

/**
 * TrendChart.tsx
 * 请求趋势双 Y 轴图：左轴 Requests，右轴 Tokens。
 * 支持 7/30/90 天范围切换与服务商筛选（本地 mock）。
 */

import React, { useMemo, useState } from 'react'
import {
  ResponsiveContainer,
  ComposedChart,
  XAxis,
  YAxis,
  Tooltip as ReTooltip,
  Legend,
  Bar,
  Line,
  CartesianGrid,
} from 'recharts'
import { TitledCard } from '@/components/common/UnifiedCard'

/** 单条趋势数据结构 */
export interface TrendPoint {
  /** 日期（MM-DD）或小时（HH:mm） */
  t: string
  /** 请求次数 */
  requests: number
  /** Token 使用量 */
  tokens: number
}

/** 服务商类型 */
type Provider = 'all' | 'openai' | 'claude' | 'gemini'

/** 生成 mock 数据（用于演示） */
function generateMock(period: 7 | 30 | 90, seed: number): TrendPoint[] {
  const out: TrendPoint[] = []
  let baseReq = 800 + (seed % 200)
  let baseTok = 15000 + ((seed * 37) % 3000)
  for (let i = period - 1; i >= 0; i--) {
    // 简单的周期波动
    const wobble = Math.sin((i / period) * Math.PI * 2) * 80
    const req = Math.max(50, Math.round(baseReq + wobble + (i % 5) * 10 - (seed % 30)))
    const tok = Math.max(600, Math.round(baseTok + wobble * 40 + (i % 7) * 120 - (seed % 700)))
    out.push({
      t: `${i + 1}d`,
      requests: req,
      tokens: tok,
    })
  }
  return out
}

/**
 * TrendChart 组件
 * - 固定样式：白卡 + 细边 + 内边距，与现有设计一致
 * - 操作区：时段切换 + 服务商筛选；按钮为自定义样式，避免引入 outline 变体限制
 */
const TrendChart: React.FC = () => {
  const [period, setPeriod] = useState<7 | 30 | 90>(7)
  const [provider, setProvider] = useState<Provider>('all')

  /** 根据 provider 与 period 生成不同的 mock 数据 */
  const data = useMemo(() => {
    const seedMap: Record<Provider, number> = { all: 1, openai: 3, claude: 7, gemini: 11 }
    return generateMock(period, seedMap[provider])
  }, [period, provider])

  /** 切换按钮样式（选中为紫色背景，未选为浅灰） */
  const tabCls = (active: boolean) =>
    [
      'inline-flex h-8 items-center rounded-lg px-3 text-xs font-medium transition-colors',
      active
        ? 'bg-violet-600 text-white'
        : 'bg-neutral-100 text-neutral-700 hover:bg-neutral-200',
    ].join(' ')

  return (
    <TitledCard
      variant="compact"
      title="请求趋势"
      headerClassName="pb-2"
      contentClassName="pt-2"
    >
      <div className="mb-3 flex flex-wrap items-center gap-2">
        <div className="flex items-center gap-1 rounded-lg bg-neutral-50 p-1">
          {[7, 30, 90].map((p) => (
            <button
              key={p}
              type="button"
              onClick={() => setPeriod(p as 7 | 30 | 90)}
              className={tabCls(period === p)}
              aria-pressed={period === p}
            >
              最近{p}天
            </button>
          ))}
        </div>
        <div className="flex items-center gap-1 rounded-lg bg-neutral-50 p-1">
          {(['all', 'openai', 'claude', 'gemini'] as Provider[]).map((pv) => (
            <button
              key={pv}
              type="button"
              onClick={() => setProvider(pv)}
              className={tabCls(provider === pv)}
              aria-pressed={provider === pv}
              title={pv === 'all' ? '全部服务商' : pv.toUpperCase()}
            >
              {pv === 'all' ? '全部' : pv.toUpperCase()}
            </button>
          ))}
        </div>
      </div>

      <div className="h-72">
        <ResponsiveContainer width="100%" height="100%">
          <ComposedChart data={data} margin={{ top: 10, right: 16, bottom: 0, left: 0 }}>
            <CartesianGrid stroke="#eee" vertical={false} />
            <XAxis dataKey="t" tick={{ fill: '#6b7280', fontSize: 12 }} />
            <YAxis
              yAxisId="left"
              tick={{ fill: '#6b7280', fontSize: 12 }}
              label={{ value: 'Requests', angle: -90, position: 'insideLeft', fill: '#6b7280' }}
            />
            <YAxis
              yAxisId="right"
              orientation="right"
              tick={{ fill: '#6b7280', fontSize: 12 }}
              label={{ value: 'Tokens', angle: 90, position: 'insideRight', fill: '#6b7280' }}
            />
            <ReTooltip formatter={(v: any) => (typeof v === 'number' ? v.toLocaleString() : v)} />
            <Legend />
            <Bar
              yAxisId="left"
              dataKey="requests"
              name="请求次数"
              fill="url(#reqGradient)"
              radius={[6, 6, 0, 0]}
            />
            <Line
              yAxisId="right"
              dataKey="tokens"
              name="Tokens"
              type="monotone"
              stroke="#6D5BD0"
              strokeWidth={2}
              dot={false}
            />
            {/* 渐变定义：请求柱使用紫色渐变 */}
            <defs>
              <linearGradient id="reqGradient" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stopColor="#8757E8" />
                <stop offset="100%" stopColor="#6D5BD0" />
              </linearGradient>
            </defs>
          </ComposedChart>
        </ResponsiveContainer>
      </div>
    </TitledCard>
  )
}

export default TrendChart

/**
 * ProviderStatus.tsx
 * 服务商状态监控列表：可用性、平均响应时延、错误率。
 */

import React from 'react'

/** 单个服务商状态 */
export interface ProviderState {
  /** 服务商名，如 OpenAI / Claude / Gemini */
  name: string
  /** 可用性（百分比） */
  availability: number
  /** 平均响应时延（ms） */
  latencyMs: number
  /** 错误率（百分比） */
  errorRate: number
  /** 在线/故障状态 */
  healthy: boolean
}

/** 组件 props */
export interface ProviderStatusProps {
  items: ProviderState[]
}

/** 圆点状态样式 */
function Dot({ healthy }: { healthy: boolean }) {
  return (
    <span
      className={[
        'inline-block h-2.5 w-2.5 rounded-full',
        healthy ? 'bg-emerald-500' : 'bg-rose-500',
      ].join(' ')}
      aria-hidden
    />
  )
}

/**
 * ProviderStatus
 * - 表格风样式但使用简洁卡片行，符合当前 UI
 */
const ProviderStatus: React.FC<ProviderStatusProps> = ({ items }) => {
  return (
    <div className="rounded-xl border border-neutral-200 bg-white p-4 shadow-sm">
      <h3 className="mb-3 text-base font-semibold text-neutral-900">服务商状态监控</h3>
      <div className="divide-y divide-neutral-100">
        <div className="grid grid-cols-12 items-center gap-2 px-2 py-2 text-xs text-neutral-500">
          <div className="col-span-4">服务商</div>
          <div className="col-span-3">可用性</div>
          <div className="col-span-3">平均响应</div>
          <div className="col-span-2 text-right">错误率</div>
        </div>
        {items.map((it) => (
          <div
            key={it.name}
            className="grid grid-cols-12 items-center gap-2 px-2 py-3 text-sm hover:bg-neutral-50"
          >
            <div className="col-span-4 flex items-center gap-2 font-medium text-neutral-800">
              <Dot healthy={it.healthy} />
              {it.name}
              <span
                className={[
                  'ml-2 rounded-md px-1.5 py-0.5 text-[10px] ring-1',
                  it.healthy
                    ? 'bg-emerald-50 text-emerald-700 ring-emerald-200'
                    : 'bg-rose-50 text-rose-700 ring-rose-200',
                ].join(' ')}
              >
                {it.healthy ? '正常' : '故障'}
              </span>
            </div>
            <div className="col-span-3 text-neutral-700">{it.availability.toFixed(3)}%</div>
            <div className="col-span-3 text-neutral-700">{it.latencyMs} ms</div>
            <div className="col-span-2 text-right text-neutral-700">{it.errorRate.toFixed(2)}%</div>
          </div>
        ))}
      </div>
    </div>
  )
}

export default ProviderStatus

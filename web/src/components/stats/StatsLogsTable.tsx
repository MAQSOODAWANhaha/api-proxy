import { Loader2 } from 'lucide-react'

import { TitledCard } from '@/components/common/UnifiedCard'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import type { LogsPage, LogItem } from '@/types/stats'
import { useTimezoneStore } from '@/store/timezone'

interface StatsLogsTableProps {
  logs: LogsPage | null
  loading?: boolean
  onPageChange: (page: number) => void
  hasFetched: boolean
}

const formatTimestamp = (timestamp: string, timezone: string) =>
  new Date(timestamp).toLocaleString('zh-CN', { timeZone: timezone, hour12: false })

const renderStatusBadge = (item: LogItem) => (
  <Badge variant={item.is_success ? 'outline' : 'destructive'} className="px-2">
    {item.status_code ?? '--'}
  </Badge>
)

export function StatsLogsTable({ logs, loading, onPageChange, hasFetched }: StatsLogsTableProps) {
  const timezone = useTimezoneStore((state) => state.timezone)

  const page = logs?.page ?? 1
  const pageSize = logs?.page_size ?? 20
  const total = logs?.total ?? 0
  const totalPages = Math.max(1, Math.ceil(total / pageSize))
  const rows: LogItem[] = logs?.items ?? []

  const handlePageChangeInternal = (nextPage: number) => {
    if (nextPage < 1 || nextPage > totalPages || nextPage === page) return
    onPageChange(nextPage)
  }

  return (
    <TitledCard
      title="调用日志"
      description="查看最近的 API 调用明细。"
      className="shadow-sm"
      contentClassName="space-y-4"
    >
      <div className="relative overflow-hidden rounded-xl border bg-card">
        {loading && (
          <div className="absolute inset-0 z-10 flex items-center justify-center bg-background/70">
            <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
          </div>
        )}
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="w-[180px]">时间 ({timezone})</TableHead>
              <TableHead className="min-w-[240px]">路径</TableHead>
              <TableHead className="w-[90px]">状态</TableHead>
              <TableHead className="w-[140px]">模型</TableHead>
              <TableHead className="w-[160px] text-right">Token</TableHead>
              <TableHead className="w-[120px] text-right">费用</TableHead>
              <TableHead className="w-[120px] text-right">执行时长</TableHead>
              <TableHead className="w-[140px]">客户端 IP</TableHead>
              <TableHead className="min-w-[260px]">用户代理</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {!hasFetched ? (
              <TableRow>
                <TableCell colSpan={9} className="py-12 text-center text-sm text-muted-foreground">
                  请输入用户 API Key 并查询后显示日志数据。
                </TableCell>
              </TableRow>
            ) : rows.length === 0 ? (
              <TableRow>
                <TableCell colSpan={9} className="py-12 text-center text-sm text-muted-foreground">
                  当前条件下暂无日志数据。
                </TableCell>
              </TableRow>
            ) : (
              rows.map((item) => {
                const method = item.method ? item.method.toUpperCase() : '--'
                const costCurrency = item.cost_currency ?? 'USD'

                return (
                  <TableRow key={item.id} className="text-sm">
                    <TableCell className="whitespace-nowrap font-medium text-foreground">
                      {formatTimestamp(item.timestamp, timezone)}
                    </TableCell>
                    <TableCell className="max-w-[260px]">
                      <div className="flex flex-col gap-1">
                        <span className="font-medium text-foreground">
                          {method} {item.path ?? '-'}
                        </span>
                        <span className="font-mono text-[11px] text-muted-foreground">
                          {item.request_id}
                        </span>
                      </div>
                    </TableCell>
                    <TableCell className="align-top">{renderStatusBadge(item)}</TableCell>
                    <TableCell className="text-muted-foreground align-top">{item.model ?? '-'}</TableCell>
                    <TableCell className="text-right text-muted-foreground align-top">
                      <div className="flex flex-col leading-tight">
                        <span className="font-medium text-foreground">
                          {item.tokens_total.toLocaleString()}
                        </span>
                        <span className="text-[11px] text-muted-foreground">
                          提示 {item.tokens_prompt.toLocaleString()} · 完成{' '}
                          {item.tokens_completion.toLocaleString()}
                        </span>
                      </div>
                    </TableCell>
                    <TableCell className="text-right text-muted-foreground align-top">
                      {item.cost != null ? `${costCurrency} ${item.cost.toFixed(4)}` : '--'}
                    </TableCell>
                    <TableCell className="text-right text-muted-foreground align-top">
                      {item.duration_ms != null ? `${item.duration_ms.toLocaleString()} ms` : '--'}
                    </TableCell>
                    <TableCell className="text-muted-foreground align-top">{item.client_ip ?? '--'}</TableCell>
                    <TableCell className="max-w-[320px] break-words text-muted-foreground align-top">
                      {item.user_agent ?? '--'}
                    </TableCell>
                  </TableRow>
                )
              })
            )}
          </TableBody>
        </Table>
      </div>

      <div className="flex flex-col items-center justify-between gap-3 text-sm text-muted-foreground sm:flex-row">
        <p>共 {total.toLocaleString()} 条记录 · 每页 {pageSize}</p>
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => handlePageChangeInternal(page - 1)}
            disabled={page <= 1 || loading}
          >
            上一页
          </Button>
          <span>
            第 {page} / {totalPages} 页
          </span>
          <Button
            variant="outline"
            size="sm"
            onClick={() => handlePageChangeInternal(page + 1)}
            disabled={page >= totalPages || loading}
          >
            下一页
          </Button>
        </div>
      </div>
    </TitledCard>
  )
}

import { useEffect, useState } from 'react'
import { Search, RotateCcw } from 'lucide-react'
import { useNavigate } from 'react-router'

import PageHeader from '@/components/common/PageHeader'
import { StatsOverview } from '@/components/stats/StatsOverview'
import { StatsTrendChart } from '@/components/stats/StatsTrendChart'
import { StatsModelShare } from '@/components/stats/StatsModelShare'
import { StatsLogsTable } from '@/components/stats/StatsLogsTable'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { useStatsStore } from '@/store/stats'
import { useTimezoneStore } from '@/store/timezone'

export default function StatsStandalonePage() {
  const {
    fetch,
    summary,
    trend,
    modelShare,
    logs,
    loading,
    error,
    filters,
    setFilters,
    hasFetched,
  } = useStatsStore()
  const timezoneStore = useTimezoneStore()
  const [apiKeyInput, setApiKeyInput] = useState(filters.userServiceKey)
  const navigate = useNavigate()

  useEffect(() => {
    if (!timezoneStore.isInitialized) {
      timezoneStore.detectTimezone()
    }
  }, [timezoneStore])

  const handlePageChange = (page: number) => {
    setFilters((draft) => {
      draft.page = page
    })
    void fetch({ page })
  }

  const handleSubmit = () => {
    setFilters((draft) => {
      draft.userServiceKey = apiKeyInput
      draft.page = 1
    })
    void fetch({ userServiceKey: apiKeyInput, page: 1 })
  }

  const handleReset = () => {
    setApiKeyInput('')
    setFilters({ userServiceKey: '', page: 1 })
  }

  return (
    <div className="min-h-screen bg-muted">
      <div className="mx-auto flex min-h-screen w-full max-w-[1400px] flex-col gap-6 px-4 pt-8 pb-16 sm:px-6 lg:px-12">
        <div className="flex flex-col gap-6">
          <div className="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
            <PageHeader
              title="用户 API Key 使用统计"
              description="输入用户 API Key 查询今日与累计请求、Token 消耗与费用情况。"
              className="flex-1"
            />
            <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-end md:gap-4">
              <Button variant="outline" size="sm" onClick={() => navigate('/dashboard')}>
                进入管理后台
              </Button>
              <div className="flex items-center gap-2">
                <div className="relative flex w-full min-w-[260px] items-center">
                  <Search className="pointer-events-none absolute left-3 h-4 w-4 text-muted-foreground" />
                  <Input
                    placeholder="请输入用户 API Key"
                    value={apiKeyInput}
                    onChange={(event) => setApiKeyInput(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === 'Enter') handleSubmit()
                    }}
                    className="pl-9"
                  />
                </div>
                <Button variant="default" size="sm" onClick={handleSubmit} disabled={loading}>
                  查询统计
                </Button>
                <Button variant="ghost" size="icon" onClick={handleReset} disabled={loading && hasFetched}>
                  <RotateCcw className="h-4 w-4" />
                </Button>
              </div>
            </div>
          </div>
          {error ? (
            <Alert variant="destructive">
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          ) : null}
        </div>

        <section className="space-y-6">
          <StatsOverview metrics={summary} loading={loading} hasFetched={hasFetched} />
          <div className="grid gap-6 lg:grid-cols-[1.6fr_1.4fr]">
            <StatsTrendChart data={trend} loading={loading} hasFetched={hasFetched} />
            <StatsModelShare data={modelShare} loading={loading} hasFetched={hasFetched} />
          </div>
         <StatsLogsTable
            logs={logs}
            loading={loading}
            onPageChange={handlePageChange}
            hasFetched={hasFetched}
          />
        </section>
      </div>
    </div>
  )
}

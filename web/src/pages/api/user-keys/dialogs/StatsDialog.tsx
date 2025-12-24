import React, { useEffect, useMemo, useState } from 'react';
import { api } from '../../../../lib/api';
import { createSafeStats, safeCurrency, safeDateTime, safeLargeNumber, safePercentage, safeResponseTime, safeTrendData } from '../../../../lib/dataValidation';
import { ApiKey } from '../types';
import { ResponsiveContainer, ComposedChart, Bar, Line, XAxis, YAxis, CartesianGrid, Tooltip as ReTooltip, Legend } from 'recharts';
import { ChartContainer, ChartTooltip, ChartTooltipContent, type ChartConfig } from '@/components/ui/chart';
import { BarChart3 } from 'lucide-react';

const StatsDialog: React.FC<{
  item: ApiKey;
  onClose: () => void;
}> = ({ item, onClose }) => {
  // 使用数据验证工具创建安全的统计数据
  const usageStats = createSafeStats(item.usage);

  // 趋势数据状态管理
  const [trendData, setTrendData] = useState<any[]>([]);
  const [trendLoading, setTrendLoading] = useState(true);
  const [detailedTrendData, setDetailedTrendData] = useState<any[]>([]);
  const [detailedTrendLoading, setDetailedTrendLoading] = useState(true);

  // 获取趋势数据
  useEffect(() => {
    const fetchTrendData = async () => {
      try {
        setTrendLoading(true);
        const response = await api.userService.getKeyTrends(item.id, {
          days: 7,
        });
        if (
          response.success &&
          response.data &&
          Array.isArray(response.data.trend_data)
        ) {
          // 转换后端数据为前端需要的格式
          const formattedData = response.data.trend_data.map((point: any) =>
            Number(point?.requests ?? 0)
          );
          setTrendData(formattedData);
        } else {
          // 如果获取失败或数据格式不对，使用空数组
          setTrendData([]);
        }
      } catch (error) {
        console.error("获取趋势数据失败:", error);
        setTrendData([]);
      } finally {
        setTrendLoading(false);
      }
    };

    fetchTrendData();
  }, [item.id]);

  // 获取7天的详细趋势数据（用于综合趋势图）
  useEffect(() => {
    const fetchDetailedTrendData = async () => {
      try {
        setDetailedTrendLoading(true);
        const response = await api.userService.getKeyTrends(item.id, {
          days: 7,
        });
        if (
          response.success &&
          response.data &&
          Array.isArray(response.data.trend_data)
        ) {
          // 转换为混合图表需要的格式
          const formattedData = response.data.trend_data.map((point: any) => ({
            date: point?.date,
            requests: Number(point?.requests ?? 0),
            tokens: Number(point?.tokens ?? point?.total_tokens ?? 0),
            successful_requests: Number(point?.successful_requests ?? 0),
            failed_requests: Number(point?.failed_requests ?? 0),
            cost: Number(point?.cost ?? point?.total_cost ?? 0),
            avg_response_time: Number(point?.avg_response_time ?? 0),
            success_rate: Number(point?.success_rate ?? 0),
          }));
          setDetailedTrendData(formattedData);
        } else {
          setDetailedTrendData([]);
        }
      } catch (error) {
        console.error("获取详细趋势数据失败:", error);
        setDetailedTrendData([]);
      } finally {
        setDetailedTrendLoading(false);
      }
    };

    fetchDetailedTrendData();
  }, [item.id]);

  const stats = {
    ...usageStats,
    // 使用真实的趋势数据
    dailyUsage: trendData.length > 0 ? trendData : safeTrendData(),
  };

  const successRateDisplay = useMemo(
    () => safePercentage(stats.successRate).toFixed(2),
    [stats.successRate]
  );

  const chartConfig = {
    requests: {
      label: "请求数",
      color: "hsl(var(--chart-1))",
    },
    tokens: {
      label: "Token消耗",
      color: "hsl(var(--chart-2))",
    },
    successful_requests: {
      label: "成功请求",
      color: "hsl(var(--chart-3))",
    },
  } satisfies ChartConfig;

  return (
    <div className="bg-white rounded-2xl p-6 w-full max-w-2xl mx-4 max-h-[80vh] overflow-y-auto border border-neutral-200 hover:shadow-sm transition-shadow">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-lg font-medium text-neutral-900">API Key 统计</h3>
        <button
          onClick={onClose}
          className="text-neutral-500 hover:text-neutral-700"
        >
          ×
        </button>
      </div>

      <div className="space-y-6">
        {/* 基本信息 */}
        <div className="grid grid-cols-2 gap-4">
          <div className="p-4 bg-neutral-50 rounded-xl">
            <div className="text-sm text-neutral-600">密钥名称</div>
            <div className="font-medium">{item.name}</div>
          </div>
          <div className="p-4 bg-neutral-50 rounded-xl">
            <div className="text-sm text-neutral-600">服务商类型</div>
            <div className="font-medium">{item.provider}</div>
          </div>
        </div>

        {/* 使用统计 */}
        <div className="grid grid-cols-4 gap-4">
          <div className="p-4 bg-violet-50 rounded-xl">
            <div className="text-sm text-violet-600">使用次数</div>
            <div className="text-2xl font-bold text-violet-900">
              {safeLargeNumber(stats.totalRequests)}
            </div>
          </div>
          <div className="p-4 bg-emerald-50 rounded-xl">
            <div className="text-sm text-emerald-600">成功率</div>
            <div className="text-2xl font-bold text-emerald-900">
              {successRateDisplay}%
            </div>
          </div>
          <div className="p-4 bg-orange-50 rounded-xl">
            <div className="text-sm text-orange-600">平均响应时间</div>
            <div className="text-2xl font-bold text-orange-900">
              {safeResponseTime(stats.avgResponseTime)}
            </div>
          </div>
          <div className="p-4 bg-blue-50 rounded-xl">
            <div className="text-sm text-blue-600">总花费</div>
            <div className="text-2xl font-bold text-blue-900">
              {safeCurrency(stats.totalCost)}
            </div>
          </div>
        </div>

        {/* 综合趋势图（柱状图+折线图） */}
        <div>
          <h4 className="text-sm font-medium text-neutral-900 mb-3">
            综合使用趋势分析
          </h4>
          <div className="h-64 w-full">
            {detailedTrendLoading ? (
              <div className="flex items-center justify-center h-full text-neutral-500">
                <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-violet-600"></div>
              </div>
            ) : detailedTrendData.length > 0 ? (
              <ChartContainer config={chartConfig} className="w-full h-full">
                <ComposedChart
                  data={detailedTrendData}
                  margin={{ top: 20, right: 30, left: 20, bottom: 20 }}
                >
                  <CartesianGrid vertical={false} />
                  <XAxis
                    dataKey="date"
                    tickFormatter={(value) => {
                      const date = new Date(value);
                      return `${date.getMonth() + 1}/${date.getDate()}`;
                    }}
                    tickLine={false}
                    axisLine={false}
                    tickMargin={8}
                  />
                  <YAxis
                    yAxisId="left"
                    tickLine={false}
                    axisLine={false}
                    tickMargin={8}
                  />
                  <YAxis
                    yAxisId="right"
                    orientation="right"
                    tickLine={false}
                    axisLine={false}
                    tickMargin={8}
                  />
                  <ChartTooltip
                    cursor={false}
                    content={<ChartTooltipContent indicator="dot" />}
                  />
                  <Legend />
                  {/* 柱状图：请求次数 */}
                  <Bar
                    yAxisId="left"
                    dataKey="requests"
                    fill="var(--color-requests)"
                    radius={[4, 4, 0, 0]}
                  />
                  {/* 折线图：Token消耗 */}
                  <Line
                    yAxisId="right"
                    type="monotone"
                    dataKey="tokens"
                    stroke="var(--color-tokens)"
                  />
                  {/* 成功请求率 */}
                  <Line
                    yAxisId="left"
                    type="monotone"
                    dataKey="successful_requests"
                    stroke="var(--color-successful_requests)"
                    strokeDasharray="3 3"
                  />
                </ComposedChart>
              </ChartContainer>
            ) : (
              <div className="flex items-center justify-center h-full text-neutral-500">
                <div className="text-center">
                  <BarChart3 className="mx-auto h-12 w-12 text-neutral-400" />
                  <div className="mt-2 text-sm">暂无趋势数据</div>
                </div>
              </div>
            )}
          </div>
        </div>

        {/* 详细统计 */}
        <div className="grid grid-cols-2 gap-4">
          <div className="p-4 bg-neutral-50 rounded-xl">
            <div className="text-sm text-neutral-600">总Token数</div>
            <div className="text-2xl font-bold text-neutral-900">
              {safeLargeNumber(stats.totalTokens)}
            </div>
          </div>
          <div className="p-4 bg-neutral-50 rounded-xl">
            <div className="text-sm text-neutral-600">最后使用时间</div>
            <div className="text-lg font-medium text-neutral-900">
              {safeDateTime(stats.lastUsedAt)}
            </div>
          </div>
        </div>

        {/* 限制配置 */}
        <div>
          <h4 className="text-sm font-medium text-neutral-900 mb-3">
            限制配置
          </h4>
          <div className="grid grid-cols-2 gap-4">
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">速率限制/分钟</div>
              <div className="font-medium">
                {(item.max_request_per_min || 0) > 0 ? `${item.max_request_per_min!.toLocaleString()} 次/分钟` : '无'}
              </div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">速率限制/天</div>
              <div className="font-medium">
                {(item.max_requests_per_day || 0) > 0 ? `${item.max_requests_per_day!.toLocaleString()} 次/天` : "无"}
              </div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">Token/天</div>
              <div className="font-medium">
                {(item.max_tokens_per_day || 0) > 0 ? `${item.max_tokens_per_day!.toLocaleString()} Token/天` : "无"}
              </div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">费用/天</div>
              <div className="font-medium">
                {Number(item.max_cost_per_day || 0) > 0
                  ? `$${Number(item.max_cost_per_day || 0).toFixed(2)}`
                  : "无"}
              </div>
            </div>
          </div>
        </div>

        {/* 调度配置 */}
        <div>
          <h4 className="text-sm font-medium text-neutral-900 mb-3">
            调度配置
          </h4>
          <div className="grid grid-cols-3 gap-4">
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">调度策略</div>
              <div className="font-medium">{item.scheduling_strategy}</div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">重试次数</div>
              <div className="font-medium">{item.retry_count}</div>
            </div>
            <div className="p-3 bg-neutral-50 rounded-lg">
              <div className="text-sm text-neutral-600">超时时间</div>
              <div className="font-medium">{item.timeout_seconds}s</div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default StatsDialog;

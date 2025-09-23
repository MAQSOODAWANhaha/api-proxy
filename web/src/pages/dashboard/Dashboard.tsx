/**
 * Dashboard.tsx
 * 仪表板首页：提供关键指标卡片与简易概览，保证首页不为空白。
 */

import React, { useState, useMemo } from 'react'
import { Activity, Timer, Coins, CheckCircle2, Calendar, ChevronDown, TrendingUp, BarChart, Loader2, AlertCircle, RefreshCw } from 'lucide-react'
import { useDashboardCards } from '../../hooks/useDashboardCards'
import { useModelsRate } from '../../hooks/useModelsRate'
import { useModelsStatistics } from '../../hooks/useModelsStatistics'
import { useTokensTrend } from '../../hooks/useTokensTrend'
import { useUserApiKeysTrend } from '../../hooks/useUserApiKeysTrend'
import { LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, Area, AreaChart, PieChart as RechartsPieChart, Pie, Cell } from 'recharts'

/** 指标项接口 */
interface StatItem {
  key: string
  label: string
  value: string
  delta: string
  icon: React.ReactNode
  color: string
}

/** 时间范围类型 */
type TimeRange = 'today' | '7days' | '30days' | 'custom'

/** 模型使用数据接口 */
interface ModelUsage {
  name: string
  count: number
  percentage: number
  cost: number
  color: string
}

/** 自定义日期范围接口 */
interface CustomDateRange {
  startDate: string
  endDate: string
}

/** 趋势图数据点接口 */
interface TrendDataPoint {
  date: string
  requests: number
  tokens: number
}

/** 趋势图显示模式 */
type TrendViewMode = 'requests' | 'tokens'

/** 指标卡片组件 */
const StatCard: React.FC<{ item: StatItem }> = ({ item }) => {
  return (
    <div className="group relative overflow-hidden rounded-2xl border border-neutral-200 bg-white p-4 shadow-sm transition hover:shadow-md">
      {/* 顶部色条 */}
      <div className="absolute inset-x-0 top-0 h-1" style={{ backgroundColor: item.color }} />
      <div className="flex items-center gap-3">
        <div
          className="flex h-10 w-10 items-center justify-center rounded-xl text-white"
          style={{ backgroundColor: item.color }}
          aria-hidden
        >
          {item.icon}
        </div>
        <div className="min-w-0">
          <div className="text-sm text-neutral-500">{item.label}</div>
          <div className="flex items-baseline gap-2">
            <div className="truncate text-xl font-semibold text-neutral-900">{item.value}</div>
            <div className="text-xs text-emerald-600">{item.delta}</div>
          </div>
        </div>
      </div>
    </div>
  )
}

/** 趋势图组件 */
const TrendChart: React.FC<{ 
  data: TrendDataPoint[] 
  viewMode: TrendViewMode
  onViewModeChange: (mode: TrendViewMode) => void
  title: string
  color: string
}> = ({ data, viewMode, onViewModeChange, title, color }) => {
  // 安全地处理数据，过滤无效值
  const validData = data.map(d => ({
    ...d,
    requests: Number.isFinite(d.requests) ? d.requests : 0,
    tokens: Number.isFinite(d.tokens) ? d.tokens : 0
  }))
  const maxValue = validData.length > 0 ? Math.max(...validData.map(d => viewMode === 'requests' ? d.requests : d.tokens)) : 0
  // 确保maxValue不为0（避免除零错误），如果所有值都是0，设置默认值
  const safeMaxValue = maxValue > 0 ? maxValue : 1
  
  // 生成SVG路径
  const generatePath = (points: number[]) => {
    if (points.length === 0) return ''
    
    const width = 600
    const height = 200
    const padding = 40
    
    const xStep = (width - padding * 2) / (points.length - 1)
    const yScale = (height - padding * 2) / safeMaxValue
    
    let path = `M ${padding} ${height - padding - points[0] * yScale}`
    
    for (let i = 1; i < points.length; i++) {
      const x = padding + i * xStep
      const y = height - padding - points[i] * yScale
      path += ` L ${x} ${y}`
    }
    
    return path
  }
  
  const currentData = validData.map(d => viewMode === 'requests' ? d.requests : d.tokens)
  const pathData = generatePath(currentData)
  
  return (
    <div className="space-y-4">
      {/* 标题和切换按钮 */}
      <div className="flex items-center justify-between">
        <h4 className="text-sm font-medium text-neutral-900">{title}</h4>
        <div className="flex rounded-lg border border-neutral-200 bg-white">
          <button
            onClick={() => onViewModeChange('requests')}
            className={`flex items-center gap-1 px-3 py-1 text-xs rounded-l-lg transition-colors ${
              viewMode === 'requests' 
                ? 'bg-violet-100 text-violet-700' 
                : 'text-neutral-600 hover:text-neutral-800'
            }`}
          >
            <BarChart size={12} />
            请求次数
          </button>
          <button
            onClick={() => onViewModeChange('tokens')}
            className={`flex items-center gap-1 px-3 py-1 text-xs rounded-r-lg transition-colors ${
              viewMode === 'tokens' 
                ? 'bg-violet-100 text-violet-700' 
                : 'text-neutral-600 hover:text-neutral-800'
            }`}
          >
            <Coins size={12} />
            Token数量
          </button>
        </div>
      </div>
      
      {/* 趋势图 */}
      <div className="relative">
        <svg width="600" height="200" className="w-full">
          {/* 网格线 */}
          <defs>
            <pattern id="grid" width="60" height="40" patternUnits="userSpaceOnUse">
              <path d="M 60 0 L 0 0 0 40" fill="none" stroke="#f3f4f6" strokeWidth="1"/>
            </pattern>
          </defs>
          <rect width="100%" height="100%" fill="url(#grid)" />
          
          {/* 趋势线 */}
          <path
            d={pathData}
            fill="none"
            stroke={color}
            strokeWidth="3"
            strokeLinecap="round"
            strokeLinejoin="round"
            className="drop-shadow-sm"
          />
          
          {/* 数据点 */}
          {currentData.map((value, index) => {
            const width = 600
            const height = 200
            const padding = 40
            const xStep = (width - padding * 2) / (currentData.length - 1)
            const yScale = (height - padding * 2) / safeMaxValue
            const x = padding + index * xStep
            const y = height - padding - value * yScale
            
            return (
              <circle
                key={index}
                cx={x}
                cy={y}
                r="4"
                fill={color}
                className="hover:r-6 transition-all cursor-pointer"
              >
                <title>{`${validData[index].date}: ${value.toLocaleString()}`}</title>
              </circle>
            )
          })}
        </svg>
        
        {/* X轴标签 */}
        <div className="flex justify-between mt-2 px-10 text-xs text-neutral-500">
          {validData.map((item, index) => (
            <span key={index} className="text-center">
              {new Date(item.date).toLocaleDateString('zh-CN', { month: 'short', day: 'numeric' })}
            </span>
          ))}
        </div>
      </div>
      
      {/* 统计信息 */}
      <div className="grid grid-cols-3 gap-4 pt-3 border-t border-neutral-100">
        <div className="text-center">
          <div className="text-lg font-bold text-neutral-900">
            {currentData[currentData.length - 1]?.toLocaleString() || 0}
          </div>
          <div className="text-xs text-neutral-500">最新值</div>
        </div>
        <div className="text-center">
          <div className="text-lg font-bold text-neutral-900">
            {Math.round(currentData.reduce((sum, val) => sum + val, 0) / currentData.length).toLocaleString()}
          </div>
          <div className="text-xs text-neutral-500">平均值</div>
        </div>
        <div className="text-center">
          <div className="text-lg font-bold text-neutral-900">
            {Math.max(...currentData).toLocaleString()}
          </div>
          <div className="text-xs text-neutral-500">峰值</div>
        </div>
      </div>
    </div>
  )
}

/** 简化的Token趋势图组件 - 使用Recharts */
const SimpleTokenChart: React.FC<{
  data: { date: string; value: number }[]
}> = ({ data }) => {
  // 安全地处理数据，过滤无效值
  const chartData = useMemo(() => {
    return data.map(d => ({
      date: d.date,
      value: Number.isFinite(d.value) ? d.value : 0,
      displayDate: new Date(d.date).toLocaleDateString('zh-CN', { month: 'short', day: 'numeric' })
    }))
  }, [data])
  
  const values = chartData.map(d => d.value)
  
  // 如果没有数据，显示空状态
  if (chartData.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-80 text-neutral-400">
        <div className="text-center">
          <div className="text-4xl mb-2">📈</div>
          <div className="text-sm">暂无Token趋势数据</div>
        </div>
      </div>
    )
  }
  
  // 格式化Token数值
  const formatTokenValue = (value: number) => {
    if (value >= 1000000) return `${(value / 1000000).toFixed(1)}M`
    if (value >= 1000) return `${(value / 1000).toFixed(1)}K`
    return value.toString()
  }

  // 自定义Tooltip
  const CustomTooltip = ({ active, payload, label }: any) => {
    if (active && payload && payload.length) {
      return (
        <div className="bg-white border border-neutral-200 text-neutral-800 text-xs rounded-md px-2 py-1.5 shadow-xl">
          <div className="font-semibold text-blue-600 text-xs leading-tight">
            {formatTokenValue(payload[0].value)}
          </div>
          <div className="text-neutral-500 text-xs leading-tight mt-0.5">
            {payload[0].payload.displayDate}
          </div>
        </div>
      )
    }
    return null
  }

  return (
    <div className="space-y-4">
      {/* 图表 */}
      <div className="h-52">
        <ResponsiveContainer width="100%" height="100%">
          <AreaChart data={chartData} margin={{ top: 5, right: 20, left: 20, bottom: 5 }}>
            <defs>
              <linearGradient id="tokenAreaGradient" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stopColor="#0ea5e9" stopOpacity={0.3}/>
                <stop offset="100%" stopColor="#0ea5e9" stopOpacity={0}/>
              </linearGradient>
            </defs>
            <CartesianGrid strokeDasharray="3 3" stroke="#f3f4f6" />
            <XAxis 
              dataKey="displayDate" 
              axisLine={false}
              tickLine={false}
              tick={{ fontSize: 12, fill: '#6b7280' }}
            />
            <YAxis 
              axisLine={false}
              tickLine={false}
              tick={{ fontSize: 12, fill: '#6b7280' }}
              tickFormatter={formatTokenValue}
            />
            <Tooltip content={<CustomTooltip />} />
            <Area
              type="monotone"
              dataKey="value"
              stroke="#0ea5e9"
              strokeWidth={3}
              fill="url(#tokenAreaGradient)"
              dot={{ fill: '#0ea5e9', strokeWidth: 2, stroke: 'white', r: 4 }}
              activeDot={{ r: 6, fill: '#0ea5e9', strokeWidth: 2, stroke: 'white' }}
            />
          </AreaChart>
        </ResponsiveContainer>
      </div>
      
      {/* 统计信息 */}
      <div className="grid grid-cols-3 gap-4 pt-3 border-t border-neutral-100">
        <div className="text-center">
          <div className="text-lg font-bold text-neutral-900">
            {formatTokenValue(values[values.length - 1] || 0)}
          </div>
          <div className="text-xs text-neutral-500">最新值</div>
        </div>
        <div className="text-center">
          <div className="text-lg font-bold text-neutral-900">
            {formatTokenValue(Math.round(values.reduce((sum, val) => sum + val, 0) / values.length))}
          </div>
          <div className="text-xs text-neutral-500">平均值</div>
        </div>
        <div className="text-center">
          <div className="text-lg font-bold text-neutral-900">
            {formatTokenValue(Math.max(...values))}
          </div>
          <div className="text-xs text-neutral-500">峰值</div>
        </div>
      </div>
    </div>
  )
}

/** 无控制按钮的趋势图组件 - 使用Recharts */
const TrendChartWithoutControls: React.FC<{
  data: TrendDataPoint[]
  viewMode: TrendViewMode
  color: string
}> = ({ data, viewMode, color }) => {
  // 安全地处理数据，过滤无效值
  const chartData = useMemo(() => {
    return data.map(d => {
      const value = viewMode === 'requests' ? d.requests : d.tokens
      return {
        date: d.date,
        value: Number.isFinite(value) ? value : 0,
        displayDate: new Date(d.date).toLocaleDateString('zh-CN', { month: 'short', day: 'numeric' })
      }
    })
  }, [data, viewMode])
  
  const values = chartData.map(d => d.value)
  
  // 如果没有数据，显示空状态
  if (chartData.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-80 text-neutral-400">
        <div className="text-center">
          <div className="text-4xl mb-2">📊</div>
          <div className="text-sm">暂无趋势数据</div>
        </div>
      </div>
    )
  }

  // 自定义Tooltip
  const CustomTooltip = ({ active, payload, label }: any) => {
    if (active && payload && payload.length) {
      return (
        <div className="bg-white border border-neutral-200 text-neutral-800 text-xs rounded-md px-2 py-1.5 shadow-xl">
          <div className="font-semibold text-purple-600 text-xs leading-tight">
            {payload[0].value.toLocaleString()}
          </div>
          <div className="text-neutral-500 text-xs leading-tight mt-0.5">
            {payload[0].payload.displayDate}
          </div>
        </div>
      )
    }
    return null
  }

  return (
    <div className="space-y-4">
      {/* 图表 */}
      <div className="h-52">
        <ResponsiveContainer width="100%" height="100%">
          <LineChart data={chartData} margin={{ top: 5, right: 20, left: 20, bottom: 5 }}>
            <CartesianGrid strokeDasharray="3 3" stroke="#f3f4f6" />
            <XAxis 
              dataKey="displayDate" 
              axisLine={false}
              tickLine={false}
              tick={{ fontSize: 12, fill: '#6b7280' }}
            />
            <YAxis 
              axisLine={false}
              tickLine={false}
              tick={{ fontSize: 12, fill: '#6b7280' }}
            />
            <Tooltip content={<CustomTooltip />} />
            <Line
              type="monotone"
              dataKey="value"
              stroke={color}
              strokeWidth={3}
              dot={{ fill: color, strokeWidth: 2, stroke: 'white', r: 4 }}
              activeDot={{ r: 6, fill: color, strokeWidth: 2, stroke: 'white' }}
            />
          </LineChart>
        </ResponsiveContainer>
      </div>
      
      {/* 统计信息 */}
      <div className="grid grid-cols-3 gap-4 pt-3 border-t border-neutral-100">
        <div className="text-center">
          <div className="text-lg font-bold text-neutral-900">
            {values[values.length - 1]?.toLocaleString() || 0}
          </div>
          <div className="text-xs text-neutral-500">最新值</div>
        </div>
        <div className="text-center">
          <div className="text-lg font-bold text-neutral-900">
            {Math.round(values.reduce((sum, val) => sum + val, 0) / values.length).toLocaleString()}
          </div>
          <div className="text-xs text-neutral-500">平均值</div>
        </div>
        <div className="text-center">
          <div className="text-lg font-bold text-neutral-900">
            {Math.max(...values).toLocaleString()}
          </div>
          <div className="text-xs text-neutral-500">峰值</div>
        </div>
      </div>
    </div>
  )
}

/** 紧凑型时间选择器组件 */
const CompactTimeRangeSelector: React.FC<{
  selectedRange: TimeRange
  customRange: CustomDateRange
  onRangeChange: (range: TimeRange) => void
  onCustomRangeChange: (range: CustomDateRange) => void
}> = ({ selectedRange, customRange, onRangeChange, onCustomRangeChange }) => {
  const [showDropdown, setShowDropdown] = useState(false)
  const [showCustomPicker, setShowCustomPicker] = useState(false)

  const timeRangeOptions = [
    { value: 'today' as TimeRange, label: '今天' },
    { value: '7days' as TimeRange, label: '最近7天' },
    { value: '30days' as TimeRange, label: '最近30天' },
    { value: 'custom' as TimeRange, label: '自定义时间' },
  ]

  const getCurrentLabel = () => {
    const option = timeRangeOptions.find(opt => opt.value === selectedRange)
    if (selectedRange === 'custom') {
      return `${customRange.startDate} 至 ${customRange.endDate}`
    }
    return option?.label || '选择时间范围'
  }

  return (
    <div className="relative">
      <button
        onClick={() => setShowDropdown(!showDropdown)}
        className="flex items-center gap-1 rounded-md border border-neutral-200 bg-white px-2 py-1 text-xs hover:bg-neutral-50"
      >
        <Calendar size={12} className="text-neutral-500" />
        <span>{getCurrentLabel()}</span>
        <ChevronDown size={10} className="text-neutral-400" />
      </button>

      {showDropdown && (
        <div className="absolute right-0 z-10 mt-1 w-48 rounded-lg border border-neutral-200 bg-white shadow-lg">
          <div className="p-1">
            {timeRangeOptions.map((option) => (
              <button
                key={option.value}
                onClick={() => {
                  onRangeChange(option.value)
                  if (option.value === 'custom') {
                    setShowCustomPicker(true)
                  } else {
                    setShowCustomPicker(false)
                  }
                  setShowDropdown(false)
                }}
                className={`w-full rounded px-3 py-2 text-left text-sm hover:bg-neutral-50 ${
                  selectedRange === option.value ? 'bg-violet-50 text-violet-700' : 'text-neutral-700'
                }`}
              >
                {option.label}
              </button>
            ))}
          </div>
        </div>
      )}

      {showCustomPicker && selectedRange === 'custom' && (
        <div className="absolute right-0 z-20 mt-1 w-80 rounded-lg border border-neutral-200 bg-white p-4 shadow-lg">
          <div className="space-y-3">
            <div>
              <label className="block text-sm font-medium text-neutral-700 mb-1">开始日期</label>
              <input
                type="date"
                value={customRange.startDate}
                onChange={(e) => onCustomRangeChange({ ...customRange, startDate: e.target.value })}
                className="w-full rounded border border-neutral-200 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-neutral-700 mb-1">结束日期</label>
              <input
                type="date"
                value={customRange.endDate}
                onChange={(e) => onCustomRangeChange({ ...customRange, endDate: e.target.value })}
                className="w-full rounded border border-neutral-200 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
              />
            </div>
            <div className="flex gap-2">
              <button
                onClick={() => setShowCustomPicker(false)}
                className="flex-1 rounded bg-neutral-100 px-3 py-2 text-sm text-neutral-600 hover:bg-neutral-200"
              >
                取消
              </button>
              <button
                onClick={() => setShowCustomPicker(false)}
                className="flex-1 rounded bg-violet-600 px-3 py-2 text-sm text-white hover:bg-violet-700"
              >
                确认
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

/** 饼图组件 - 使用Recharts实现 */
const PieChart: React.FC<{ data: ModelUsage[] }> = ({ data }) => {
  const total = data.reduce((sum, item) => sum + item.count, 0)

  // 智能处理模型数据显示
  const processedData = useMemo(() => {
    // 按使用量排序
    const sortedData = [...data].sort((a, b) => b.count - a.count)
    
    // 如果模型数量少于等于4个，直接显示全部
    if (sortedData.length <= 4) return sortedData
    
    // 如果有5-6个模型，全部显示
    if (sortedData.length <= 6) return sortedData
    
    // 如果超过6个模型，显示前5个，其余合并为"其他"
    const topModels = sortedData.slice(0, 5)
    const otherModels = sortedData.slice(5)
    
    // 计算"其他"的占比，如果太小（<3%）则合并到前一个模型
    const otherTotal = otherModels.reduce((sum, item) => sum + item.count, 0)
    const otherPercentage = (otherTotal / total) * 100
    
    if (otherPercentage < 3) {
      // 如果"其他"占比太小，显示前6个，不显示"其他"
      return sortedData.slice(0, 6)
    }
    
    const otherCost = otherModels.reduce((sum, item) => sum + item.cost, 0)
    
    return [
      ...topModels,
      {
        name: `其他 (${otherModels.length}个)`,
        count: otherTotal,
        cost: otherCost,
        percentage: otherPercentage,
        color: '#6b7280'
      }
    ]
  }, [data, total])

  // 检查是否有数据
  if (!data.length || total === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-80 text-neutral-400">
        <div className="text-center">
          <div className="text-4xl mb-2">📊</div>
          <div className="text-sm">暂无模型使用数据</div>
        </div>
      </div>
    )
  }

  // 动态调整图例布局：少量模型用单列，多模型用双列
  const legendCols = processedData.length <= 3 ? 1 : 2

  // 自定义Tooltip
  const CustomTooltip = ({ active, payload }: any) => {
    if (active && payload && payload.length) {
      const data = payload[0].payload
      return (
        <div className="bg-white border border-neutral-200 rounded-lg p-3 shadow-lg">
          <div className="flex items-center gap-2 mb-2">
            <div 
              className="w-3 h-3 rounded-full"
              style={{ backgroundColor: data.color }}
            />
            <span className="font-medium text-neutral-900">{data.name}</span>
          </div>
          <div className="text-sm space-y-1">
            <div className="flex justify-between gap-4">
              <span className="text-neutral-600">请求次数:</span>
              <span className="font-medium">{data.count.toLocaleString()}</span>
            </div>
            <div className="flex justify-between gap-4">
              <span className="text-neutral-600">占比:</span>
              <span className="font-medium">{data.percentage.toFixed(1)}%</span>
            </div>
            {data.cost > 0 && (
              <div className="flex justify-between gap-4">
                <span className="text-neutral-600">成本:</span>
                <span className="font-medium">${data.cost.toFixed(2)}</span>
              </div>
            )}
          </div>
        </div>
      )
    }
    return null
  }

  return (
    <div className="flex flex-col items-center gap-6">
      {/* Recharts 饼图 */}
      <div className="relative">
        <ResponsiveContainer width={320} height={320}>
          <RechartsPieChart>
            <Pie
              data={processedData}
              cx={160}
              cy={160}
              innerRadius={60}
              outerRadius={120}
              paddingAngle={2}
              dataKey="count"
              stroke="none"
            >
              {processedData.map((entry, index) => (
                <Cell 
                  key={`cell-${index}`} 
                  fill={entry.color}
                  style={{
                    filter: 'drop-shadow(0 2px 4px rgba(0,0,0,0.1))',
                    cursor: 'pointer'
                  }}
                />
              ))}
            </Pie>
            <Tooltip content={<CustomTooltip />} />
          </RechartsPieChart>
        </ResponsiveContainer>
        
        {/* 中心显示总数 */}
        <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
          <div className="text-center">
            <div className="text-3xl font-bold text-neutral-900">{total.toLocaleString()}</div>
            <div className="text-sm text-neutral-500 mt-1">总请求数</div>
          </div>
        </div>
      </div>
      
      {/* 图例 */}
      <div className={`w-full grid gap-2 ${legendCols === 1 ? 'grid-cols-1' : 'grid-cols-2'}`}>
        {processedData.map((item, index) => (
          <div key={index} className="flex items-center gap-2">
            <div
              className="h-3 w-3 rounded-full flex-shrink-0"
              style={{ backgroundColor: item.color }}
            />
            <div className="flex-1 min-w-0">
              <div className="flex items-center justify-between">
                <span 
                  className="text-sm font-medium text-neutral-700 truncate" 
                  title={item.name}
                >
                  {item.name}
                </span>
                <span className="text-sm text-neutral-500 ml-2 flex-shrink-0">
                  {item.percentage.toFixed(1)}%
                </span>
              </div>
            </div>
          </div>
        ))}
      </div>
      
      {/* 如果显示了"其他"分类，添加提示 */}
      {processedData.some(item => item.name.includes('其他')) && (
        <div className="text-xs text-neutral-400 text-center">
          * "其他"包含使用量较少的模型，详情请查看右侧统计列表
        </div>
      )}
    </div>
  )
}

/** 模型统计列表组件 */
const ModelStatsList: React.FC<{ data: ModelUsage[] }> = ({ data }) => {
  const [showAll, setShowAll] = useState(false)
  const sortedData = [...data].sort((a, b) => b.count - a.count)
  
  // 默认显示前5个，可展开查看全部
  const displayData = showAll ? sortedData : sortedData.slice(0, 5)
  const hasMore = sortedData.length > 5

  return (
    <div className="space-y-3">
      <div className="max-h-96 overflow-y-auto space-y-3">
        {displayData.map((item, index) => (
          <div key={index} className="flex items-center justify-between rounded-lg border border-neutral-100 p-3 hover:bg-neutral-50 transition-colors">
            <div className="flex items-center gap-3">
              <div
                className="h-4 w-4 rounded-full flex-shrink-0"
                style={{ backgroundColor: item.color }}
              />
              <div className="min-w-0">
                <div className="font-medium text-neutral-900 truncate">{item.name}</div>
                <div className="text-sm text-neutral-500">{item.count.toLocaleString()} 次请求</div>
              </div>
            </div>
            <div className="text-right flex-shrink-0">
              <div className="font-medium text-neutral-900">${item.cost.toFixed(2)}</div>
              <div className="text-sm text-neutral-500">{item.percentage.toFixed(1)}%</div>
            </div>
          </div>
        ))}
      </div>
      
      {hasMore && (
        <div className="pt-3 mt-2 border-t border-neutral-100">
          <button
            onClick={() => setShowAll(!showAll)}
            className="w-full flex items-center justify-center gap-2 py-3 text-sm text-neutral-600 hover:text-neutral-800 hover:bg-neutral-50 rounded-lg transition-colors"
          >
            <span>{showAll ? '收起' : `查看全部 ${sortedData.length} 个模型`}</span>
            <svg 
              className={`h-4 w-4 transition-transform ${showAll ? 'rotate-180' : ''}`} 
              fill="none" 
              viewBox="0 0 24 24" 
              stroke="currentColor"
            >
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
            </svg>
          </button>
        </div>
      )}
    </div>
  )
}

/**
 * DashboardPage
 * - 4 个指标卡
 * - 欢迎区 + 图表
 */
/** 带独立时间筛选的饼图组件 */
const PieChartWithTimeFilter: React.FC = () => {
  const [selectedTimeRange, setSelectedTimeRange] = useState<TimeRange>('7days')
  const [customDateRange, setCustomDateRange] = useState<CustomDateRange>({
    startDate: new Date(Date.now() - 7 * 24 * 60 * 60 * 1000).toISOString().split('T')[0],
    endDate: new Date().toISOString().split('T')[0]
  })

  // 根据时间范围计算API参数
  const apiParams = useMemo(() => {
    let range = selectedTimeRange
    let start: string | undefined
    let end: string | undefined

    if (selectedTimeRange === 'custom') {
      range = 'custom'
      start = customDateRange.startDate
      end = customDateRange.endDate
    }

    return { range, start, end }
  }, [selectedTimeRange, customDateRange])

  // 使用真实的后端数据
  const { modelsRate, isLoading, error } = useModelsRate(apiParams.range, apiParams.start, apiParams.end)

  // 转换后端数据为组件需要的格式
  const modelData = useMemo(() => {
    if (!modelsRate?.model_usage) return []

    // 为每个模型分配颜色
    const colors = [
      '#7c3aed', '#0ea5e9', '#10b981', '#f59e0b', '#ef4444',
      '#8b5cf6', '#06b6d4', '#84cc16', '#f97316', '#ec4899',
      '#14b8a6', '#a855f7', '#f43f5e', '#22c55e', '#3b82f6'
    ]

    return modelsRate.model_usage.map((item, index) => ({
      name: item.model,
      count: item.usage,
      percentage: (item.usage / modelsRate.model_usage.reduce((sum, m) => sum + m.usage, 0)) * 100,
      cost: item.cost || 0,
      color: colors[index % colors.length]
    }))
  }, [modelsRate])

  return (
    <div className="rounded-2xl border border-neutral-200 bg-white p-6">
      <div className="mb-4 flex items-center justify-between">
        <h3 className="text-sm font-medium text-neutral-900">模型使用占比</h3>
        <CompactTimeRangeSelector
          selectedRange={selectedTimeRange}
          customRange={customDateRange}
          onRangeChange={setSelectedTimeRange}
          onCustomRangeChange={setCustomDateRange}
        />
      </div>
      
      {/* 加载状态 */}
      {isLoading && (
        <div className="flex items-center justify-center h-80">
          <div className="flex items-center gap-2 text-neutral-500">
            <Loader2 className="h-5 w-5 animate-spin" />
            <span className="text-sm">加载模型使用数据...</span>
          </div>
        </div>
      )}

      {/* 错误状态 */}
      {error && !isLoading && (
        <div className="flex items-center justify-center h-80 text-neutral-400">
          <div className="text-center">
            <AlertCircle className="h-8 w-8 mx-auto mb-2 text-red-400" />
            <div className="text-sm text-red-600">{error}</div>
          </div>
        </div>
      )}

      {/* 数据显示 */}
      {!isLoading && !error && <PieChart data={modelData} />}
    </div>
  )
}

/** 带独立时间筛选的统计列表组件 */
const ModelStatsListWithTimeFilter: React.FC = () => {
  const [selectedTimeRange, setSelectedTimeRange] = useState<TimeRange>('7days')
  const [customDateRange, setCustomDateRange] = useState<CustomDateRange>({
    startDate: new Date(Date.now() - 7 * 24 * 60 * 60 * 1000).toISOString().split('T')[0],
    endDate: new Date().toISOString().split('T')[0]
  })

  // 根据时间范围计算API参数
  const apiParams = useMemo(() => {
    let range = selectedTimeRange
    let start: string | undefined
    let end: string | undefined

    if (selectedTimeRange === 'custom') {
      range = 'custom'
      start = customDateRange.startDate
      end = customDateRange.endDate
    }

    return { range, start, end }
  }, [selectedTimeRange, customDateRange])

  // 使用真实的后端数据
  const { modelsStatistics, isLoading, error } = useModelsStatistics(apiParams.range, apiParams.start, apiParams.end)

  // 转换后端数据为组件需要的格式
  const modelData = useMemo(() => {
    if (!modelsStatistics?.model_usage) return []

    // 为每个模型分配颜色
    const colors = [
      '#7c3aed', '#0ea5e9', '#10b981', '#f59e0b', '#ef4444',
      '#8b5cf6', '#06b6d4', '#84cc16', '#f97316', '#ec4899',
      '#14b8a6', '#a855f7', '#f43f5e', '#22c55e', '#3b82f6'
    ]

    return modelsStatistics.model_usage.map((item, index) => ({
      name: item.model,
      count: item.usage,
      percentage: item.percentage,
      cost: item.cost || 0,
      color: colors[index % colors.length]
    }))
  }, [modelsStatistics])

  return (
    <div className="rounded-2xl border border-neutral-200 bg-white p-6">
      <div className="mb-4 flex items-center justify-between">
        <h3 className="text-sm font-medium text-neutral-900">模型使用统计</h3>
        <CompactTimeRangeSelector
          selectedRange={selectedTimeRange}
          customRange={customDateRange}
          onRangeChange={setSelectedTimeRange}
          onCustomRangeChange={setCustomDateRange}
        />
      </div>
      
      {/* 加载状态 */}
      {isLoading && (
        <div className="flex items-center justify-center h-80">
          <div className="flex items-center gap-2 text-neutral-500">
            <Loader2 className="h-5 w-5 animate-spin" />
            <span className="text-sm">加载模型统计数据...</span>
          </div>
        </div>
      )}

      {/* 错误状态 */}
      {error && !isLoading && (
        <div className="flex items-center justify-center h-80 text-neutral-400">
          <div className="text-center">
            <AlertCircle className="h-8 w-8 mx-auto mb-2 text-red-400" />
            <div className="text-sm text-red-600">{error}</div>
          </div>
        </div>
      )}

      {/* 数据显示 */}
      {!isLoading && !error && (
        modelData.length > 0 ? (
          <ModelStatsList data={modelData} />
        ) : (
          <div className="flex flex-col items-center justify-center h-80 text-neutral-400">
            <div className="text-center">
              <div className="text-4xl mb-2">📋</div>
              <div className="text-sm">暂无模型统计数据</div>
            </div>
          </div>
        )
      )}
    </div>
  )
}

/** Token使用趋势图组件 */
const TokenTrendChart: React.FC = () => {
  // 使用真实的后端数据
  const { tokensTrend, isLoading, error } = useTokensTrend()

  // 转换后端数据为组件需要的格式
  const chartData = useMemo(() => {
    if (!tokensTrend?.token_usage) return []

    return tokensTrend.token_usage.map(item => ({
      date: item.timestamp,
      value: item.tokens_prompt + item.tokens_completion + item.cache_create_tokens + item.cache_read_tokens
    }))
  }, [tokensTrend])

  return (
    <div className="rounded-2xl border border-neutral-200 bg-white p-6">
      <div className="mb-4">
        <h3 className="text-sm font-medium text-neutral-900">Token使用趋势</h3>
        <p className="text-xs text-neutral-500 mt-1">最近30天Token消耗数量</p>
      </div>
      
      {/* 加载状态 */}
      {isLoading && (
        <div className="flex items-center justify-center h-80">
          <div className="flex items-center gap-2 text-neutral-500">
            <Loader2 className="h-5 w-5 animate-spin" />
            <span className="text-sm">加载Token趋势数据...</span>
          </div>
        </div>
      )}

      {/* 错误状态 */}
      {error && !isLoading && (
        <div className="flex items-center justify-center h-80 text-neutral-400">
          <div className="text-center">
            <AlertCircle className="h-8 w-8 mx-auto mb-2 text-red-400" />
            <div className="text-sm text-red-600">{error}</div>
          </div>
        </div>
      )}

      {/* 数据显示 */}
      {!isLoading && !error && (
        chartData.length > 0 ? (
          <SimpleTokenChart data={chartData} />
        ) : (
          <div className="flex flex-col items-center justify-center h-80 text-neutral-400">
            <div className="text-center">
              <div className="text-4xl mb-2">📈</div>
              <div className="text-sm">暂无Token趋势数据</div>
            </div>
          </div>
        )
      )}
    </div>
  )
}

/** 用户API Keys使用趋势图组件 */
const UserApiKeysTrendChart: React.FC = () => {
  const [viewMode, setViewMode] = useState<TrendViewMode>('requests')

  // 使用真实的后端数据，根据模式切换接口类型
  const { trendData, isLoading, error, currentType, switchTrendType } = useUserApiKeysTrend(
    viewMode === 'requests' ? 'request' : 'token'
  )

  // 当视图模式变化时，切换API接口类型
  const handleViewModeChange = (mode: TrendViewMode) => {
    setViewMode(mode)
    switchTrendType(mode === 'requests' ? 'request' : 'token')
  }

  // 转换后端数据为组件需要的格式
  const chartData = useMemo(() => {
    if (!trendData) return []

    if (currentType === 'request' && 'request_usage' in trendData) {
      return trendData.request_usage.map(item => ({
        date: item.timestamp,
        requests: item.request,
        tokens: 0 // 在请求模式下，tokens设为0
      }))
    } else if (currentType === 'token' && 'token_usage' in trendData) {
      return trendData.token_usage.map(item => ({
        date: item.timestamp,
        requests: 0, // 在token模式下，requests设为0
        tokens: item.total_token
      }))
    }

    return []
  }, [trendData, currentType])

  return (
    <div className="rounded-2xl border border-neutral-200 bg-white p-6">
      <div className="mb-4 flex items-center justify-between">
        <div>
          <h3 className="text-sm font-medium text-neutral-900">用户API Keys使用趋势</h3>
          <p className="text-xs text-neutral-500 mt-1">最近30天数据</p>
        </div>
        
        {/* 切换按钮移动到右上方 */}
        <div className="flex rounded-lg border border-neutral-200 bg-white">
          <button
            onClick={() => handleViewModeChange('requests')}
            className={`flex items-center gap-1 px-3 py-1 text-xs rounded-l-lg transition-colors ${
              viewMode === 'requests' 
                ? 'bg-violet-100 text-violet-700' 
                : 'text-neutral-600 hover:text-neutral-800'
            }`}
          >
            <BarChart size={12} />
            请求次数
          </button>
          <button
            onClick={() => handleViewModeChange('tokens')}
            className={`flex items-center gap-1 px-3 py-1 text-xs rounded-r-lg transition-colors ${
              viewMode === 'tokens' 
                ? 'bg-violet-100 text-violet-700' 
                : 'text-neutral-600 hover:text-neutral-800'
            }`}
          >
            <Coins size={12} />
            Token数量
          </button>
        </div>
      </div>
      
      {/* 加载状态 */}
      {isLoading && (
        <div className="flex items-center justify-center h-80">
          <div className="flex items-center gap-2 text-neutral-500">
            <Loader2 className="h-5 w-5 animate-spin" />
            <span className="text-sm">加载用户API Keys趋势数据...</span>
          </div>
        </div>
      )}

      {/* 错误状态 */}
      {error && !isLoading && (
        <div className="flex items-center justify-center h-80 text-neutral-400">
          <div className="text-center">
            <AlertCircle className="h-8 w-8 mx-auto mb-2 text-red-400" />
            <div className="text-sm text-red-600">{error}</div>
          </div>
        </div>
      )}

      {/* 数据显示 */}
      {!isLoading && !error && (
        chartData.length > 0 ? (
          <TrendChartWithoutControls
            data={chartData}
            viewMode={viewMode}
            color="#7c3aed"
          />
        ) : (
          <div className="flex flex-col items-center justify-center h-80 text-neutral-400">
            <div className="text-center">
              <div className="text-4xl mb-2">📊</div>
              <div className="text-sm">暂无用户API Keys趋势数据</div>
            </div>
          </div>
        )
      )}
    </div>
  )
}

const DashboardPage: React.FC = () => {
  // 使用自定义hook获取仪表板数据
  const { cards, isLoading, error, refresh, lastUpdated } = useDashboardCards()

  // 图标映射
  const iconMap: Record<string, React.ReactNode> = {
    requests: <Activity size={18} />,
    tokens: <Coins size={18} />,
    latency: <Timer size={18} />,
    success: <CheckCircle2 size={18} />
  }

  // 将API数据转换为StatItem格式（保持UI组件不变）
  const stats: StatItem[] = useMemo(() => {
    return cards.map(card => ({
      key: card.key,
      label: card.label,
      value: card.value,
      delta: card.delta,
      icon: iconMap[card.key] || <Activity size={18} />,
      color: card.color
    }))
  }, [cards])

  return (
    <div className="w-full">
      {/* 欢迎区 */}
      <section className="mb-6 rounded-2xl border border-neutral-200 bg-gradient-to-r from-violet-50 to-indigo-50 p-5">
        <h2 className="text-lg font-semibold text-neutral-900">欢迎回来 👋</h2>
        <p className="mt-1 text-sm text-neutral-600">
          这里是系统运行概览与关键指标。更多分析请前往各功能页面。
        </p>
      </section>

      {/* 指标卡片 */}
      <section className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
        {/* 加载状态 */}
        {isLoading && (
          <>
            {[1, 2, 3, 4].map((i) => (
              <div key={i} className="group relative overflow-hidden rounded-2xl border border-neutral-200 bg-white p-4 shadow-sm">
                <div className="flex items-center gap-3">
                  <div className="h-10 w-10 rounded-xl bg-neutral-100 animate-pulse"></div>
                  <div className="min-w-0 flex-1">
                    <div className="h-4 w-16 bg-neutral-100 rounded animate-pulse mb-2"></div>
                    <div className="h-6 w-20 bg-neutral-100 rounded animate-pulse"></div>
                  </div>
                </div>
              </div>
            ))}
          </>
        )}

        {/* 错误状态 */}
        {error && !isLoading && (
          <div className="lg:col-span-4 sm:col-span-2 col-span-1">
            <div className="rounded-2xl border border-red-200 bg-red-50 p-4">
              <div className="flex items-center gap-3">
                <AlertCircle className="h-5 w-5 text-red-600 flex-shrink-0" />
                <div className="flex-1">
                  <h3 className="text-sm font-medium text-red-800">加载仪表板数据失败</h3>
                  <p className="text-sm text-red-600 mt-1">{error}</p>
                </div>
                <button
                  onClick={refresh}
                  className="flex items-center gap-2 px-3 py-1 text-sm text-red-700 border border-red-300 rounded-lg hover:bg-red-100 transition-colors"
                >
                  <RefreshCw className="h-4 w-4" />
                  重试
                </button>
              </div>
            </div>
          </div>
        )}

        {/* 正常数据显示 */}
        {!isLoading && !error && stats.map((s) => (
          <StatCard key={s.key} item={s} />
        ))}

        {/* 有错误但仍显示默认数据 */}
        {error && !isLoading && stats.length > 0 && stats.map((s) => (
          <StatCard key={s.key} item={s} />
        ))}
      </section>

      {/* 模型使用分析 - 2列布局 */}
      <section className="mt-6 grid grid-cols-1 gap-4 lg:grid-cols-2">
        <PieChartWithTimeFilter />
        <ModelStatsListWithTimeFilter />
      </section>

      {/* 趋势分析 - 每个图表独占一行 */}
      <section className="mt-6 space-y-4">
        <TokenTrendChart />
        <UserApiKeysTrendChart />
      </section>
    </div>
  )
}

export default DashboardPage

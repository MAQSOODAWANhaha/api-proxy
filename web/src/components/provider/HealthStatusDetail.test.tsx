/**
 * HealthStatusDetail 组件测试示例
 * 这个文件展示了如何使用 HealthStatusDetail 组件
 */

import React from 'react'
import HealthStatusDetail from './HealthStatusDetail'

// 测试数据示例
const testCases = [
  {
    name: "健康状态 - 显示窗口信息",
    health_status_detail: JSON.stringify({
      data: {
        primary: {
          used_percent: 45.5,
          window_seconds: 3600,
          resets_at: Math.floor(Date.now() / 1000) + 1800
        },
        secondary: {
          used_percent: 25.2,
          window_seconds: 86400,
          resets_at: Math.floor(Date.now() / 1000) + 43200
        }
      },
      updated_at: new Date().toISOString()
    }),
    health_status: "healthy" as const
  },
  {
    name: "健康状态 - 无详细数据",
    health_status_detail: null,
    health_status: "healthy" as const
  },
  {
    name: "限流状态 - 显示429错误",
    health_status_detail: JSON.stringify({
      data: {
        error: {
          type: "rate_limit_exceeded",
          message: "You exceeded your current quota, please check your plan and billing details. For more information on this error, check the documentation.",
          plan_type: "Pay-as-you-go",
          resets_in_seconds: 3600
        }
      },
      updated_at: new Date().toISOString()
    }),
    health_status: "rate_limited" as const
  },
  {
    name: "错误状态 - 显示错误信息",
    health_status_detail: JSON.stringify({
      data: {
        error: {
          type: "access_denied",
          message: "Invalid API key provided",
          plan_type: null
        }
      },
      updated_at: new Date().toISOString()
    }),
    health_status: "unhealthy" as const
  },
  {
    name: "警告状态 - 无详细信息",
    health_status_detail: null,
    health_status: "unhealthy" as const
  }
]

/**
 * 测试组件展示
 */
export const HealthStatusDetailExamples: React.FC = () => {
  return (
    <div className="p-6 space-y-6 max-w-4xl mx-auto">
      <h2 className="text-2xl font-bold mb-6">健康状态详情组件测试</h2>

      <div className="space-y-8">
        {testCases.map((testCase, index) => (
          <div key={index} className="border rounded-lg p-4">
            <h3 className="text-lg font-semibold mb-4">{testCase.name}</h3>
            <div className="text-sm text-gray-600 mb-4">
              <pre>{JSON.stringify(JSON.parse(testCase.health_status_detail || '{}'), null, 2)}</pre>
            </div>
            <div className="border-t pt-4">
              <HealthStatusDetail
                health_status_detail={testCase.health_status_detail}
                health_status={testCase.health_status}
              />
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}

export default HealthStatusDetailExamples

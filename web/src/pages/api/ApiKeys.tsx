/**
 * ApiKeys.tsx
 * API 密钥管理入口页：引导进入“用户API Keys”“账号API Keys”子页。
 */

import React from 'react'
import { useNavigate } from 'react-router'
import { KeyRound, KeySquare } from 'lucide-react'

/** 入口卡片组件 */
const EntryCard: React.FC<{
  title: string
  desc: string
  icon: React.ReactNode
  onClick: () => void
}> = ({ title, desc, icon, onClick }) => {
  return (
    <button
      type="button"
      onClick={onClick}
      className="group flex w-full items-start gap-4 rounded-2xl border border-neutral-200 bg-white p-5 text-left shadow-sm transition hover:shadow-md focus:outline-none focus-visible:ring-2 focus-visible:ring-violet-500/40"
      aria-label={title}
    >
      <div className="rounded-xl bg-violet-50 p-3 text-violet-600">{icon}</div>
      <div className="min-w-0">
        <div className="truncate text-base font-semibold text-neutral-900">{title}</div>
        <div className="mt-1 text-sm text-neutral-600">{desc}</div>
      </div>
    </button>
  )
}

/** 页面主组件 */
const ApiKeysPage: React.FC = () => {
  const navigate = useNavigate()
  return (
    <div className="w-full">
      <div className="mb-6">
        <h2 className="text-lg font-semibold text-neutral-900">API 密钥管理</h2>
        <p className="mt-1 text-sm text-neutral-600">选择要管理的密钥类型。</p>
      </div>
      <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
        <EntryCard
          title="用户API Keys"
          desc="面向最终用户的访问密钥。"
          icon={<KeyRound size={18} />}
          onClick={() => navigate('/api/user-keys')}
        />
        <EntryCard
          title="账号API Keys"
          desc="对接上游模型服务商的密钥。"
          icon={<KeySquare size={18} />}
          onClick={() => navigate('/api/provider-keys')}
        />
      </div>
    </div>
  )
}

export default ApiKeysPage

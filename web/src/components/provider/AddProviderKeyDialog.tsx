/**
 * AddProviderKeyDialog.tsx
 * 账号 API Key 创建/编辑弹窗：包含名称、服务商、API Key、权重、请求/Token 限制与启用开关。
 * 使用 shadcn UI 的 Dialog、Input、Select、Switch、Button 实现。
 * 扩展：支持 create/edit 双模式；支持初始值回填与更新回调。
 */

import React, { useEffect, useMemo, useState } from 'react'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectTrigger,
  SelectValue,
  SelectContent,
  SelectItem,
} from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import NumberStepper from './NumberStepper'
import { toast } from 'sonner'

/** 服务商选项 */
export interface ProviderOption {
  /** 唯一值 */
  value: 'openai' | 'claude' | 'gemini' | 'custom'
  /** 展示文本 */
  label: string
}

/** 表单数据结构 */
export interface AddProviderKeyForm {
  name: string
  provider?: ProviderOption['value']
  apiKey: string
  weight: number
  rateLimitPerMin: number
  tokenLimitPerDay: number
  enabled: boolean
}

/** 组件属性（支持创建/编辑） */
export interface AddProviderKeyDialogProps {
  /** 是否打开 */
  open: boolean
  /** 打开状态变更 */
  onOpenChange: (open: boolean) => void
  /** 创建成功回调（可触发表格刷新） */
  onCreated?: (data: AddProviderKeyForm) => void

  /** 模式：创建或编辑（默认创建） */
  mode?: 'create' | 'edit'
  /** 编辑时的初始值（不传则使用默认值） */
  initialData?: Partial<AddProviderKeyForm>
  /** 编辑对象ID（编辑模式下必传） */
  editingId?: string
  /** 编辑成功回调 */
  onUpdated?: (id: string, data: AddProviderKeyForm) => void
}

/**
 * AddProviderKeyDialog 组件
 * - 提供受控弹窗形式的账号密钥创建/编辑表单。
 */
const AddProviderKeyDialog: React.FC<AddProviderKeyDialogProps> = ({
  open,
  onOpenChange,
  onCreated,
  mode = 'create',
  initialData,
  editingId,
  onUpdated,
}) => {
  // 表单默认值
  const defaultForm: AddProviderKeyForm = {
    name: '',
    provider: undefined,
    apiKey: '',
    weight: 1,
    rateLimitPerMin: 0,
    tokenLimitPerDay: 0,
    enabled: true,
  }

  // 表单状态（简化实现，满足必填/非负数等校验）
  const [form, setForm] = useState<AddProviderKeyForm>(defaultForm)
  const [submitting, setSubmitting] = useState(false)

  /** 打开时回填初始值（编辑模式） */
  useEffect(() => {
    if (open) {
      if (mode === 'edit' && initialData) {
        setForm({
          ...defaultForm,
          ...initialData,
        })
      } else {
        setForm(defaultForm)
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, mode, JSON.stringify(initialData)])

  /** 可选服务商 */
  const providers: ProviderOption[] = useMemo(
    () => [
      { value: 'openai', label: 'OpenAI' },
      { value: 'claude', label: 'Anthropic Claude' },
      { value: 'gemini', label: 'Google Gemini' },
      { value: 'custom', label: '自定义（兼容OpenAI格式）' },
    ],
    [],
  )

  /** 更新字段的便捷函数 */
  const setField = <K extends keyof AddProviderKeyForm>(key: K, value: AddProviderKeyForm[K]) => {
    setForm((prev) => ({ ...prev, [key]: value }))
  }

  /** 关闭弹窗 */
  const close = () => {
    onOpenChange(false)
  }

  /** 简单校验并提交 */
  const handleSubmit = async () => {
    // 最小校验：名称、服务商、API Key 必填；数值非负（权重 >= 1）
    if (!form.name.trim()) {
      toast.error('请填写密钥名称')
      return
    }
    if (!form.provider) {
      toast.error('请选择服务商类型')
      return
    }
    if (!form.apiKey.trim()) {
      toast.error('请输入 API 密钥')
      return
    }
    if (form.weight < 1) {
      toast.error('权重不能小于 1')
      return
    }
    if (form.rateLimitPerMin < 0 || form.tokenLimitPerDay < 0) {
      toast.error('限制数值不能为负数')
      return
    }

    try {
      setSubmitting(true)
      // 在此调用后端接口（占位实现）
      await new Promise((r) => setTimeout(r, 400))

      if (mode === 'edit') {
        if (!editingId) {
          toast.error('缺少编辑对象 ID')
          return
        }
        onUpdated?.(editingId, form)
        toast.success('保存成功')
      } else {
        onCreated?.(form)
        toast.success('创建成功')
      }
      close()
    } catch {
      toast.error(mode === 'edit' ? '保存失败，请稍后重试' : '创建失败，请稍后重试')
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[560px]">
        <DialogHeader>
          <DialogTitle>{mode === 'edit' ? '编辑密钥' : '添加密钥'}</DialogTitle>
        </DialogHeader>

        {/* 表单主体 */}
        <div className="mt-2 grid grid-cols-1 gap-4">
          {/* 密钥名称 */}
          <div className="space-y-2">
            <Label htmlFor="provider-name">
              <span className="text-red-500 mr-1">*</span>密钥名称
            </Label>
            <Input
              id="provider-name"
              placeholder="请输入密钥名称"
              value={form.name}
              onChange={(e) => setField('name', e.target.value)}
            />
          </div>

          {/* 服务商类型 */}
          <div className="space-y-2">
            <Label htmlFor="provider-type">
              <span className="text-red-500 mr-1">*</span>服务商类型
            </Label>
            <Select
              value={form.provider}
              onValueChange={(v: ProviderOption['value']) => setField('provider', v)}
            >
              <SelectTrigger id="provider-type">
                <SelectValue placeholder="请选择服务商" />
              </SelectTrigger>
              <SelectContent>
                {providers.map((p) => (
                  <SelectItem key={p.value} value={p.value}>
                    {p.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          {/* API 密钥 */}
          <div className="space-y-2">
            <Label htmlFor="provider-api">
              <span className="text-red-500 mr-1">*</span>API密钥
            </Label>
            <Input
              id="provider-api"
              type="password"
              placeholder="请输入API密钥"
              value={form.apiKey}
              onChange={(e) => setField('apiKey', e.target.value)}
            />
          </div>

          {/* 权重 */}
          <div className="space-y-2">
            <Label>
              <span className="text-red-500 mr-1">*</span>权重
            </Label>
            <NumberStepper value={form.weight} onChange={(v) => setField('weight', v)} min={1} step={1} />
          </div>

          {/* 请求限制/分钟 */}
          <div className="space-y-2">
            <Label>请求限制/分钟</Label>
            <NumberStepper
              value={form.rateLimitPerMin}
              onChange={(v) => setField('rateLimitPerMin', v)}
              min={0}
              step={1}
            />
          </div>

          {/* Token 限制/天 */}
          <div className="space-y-2">
            <Label>Token限制/天</Label>
            <NumberStepper
              value={form.tokenLimitPerDay}
              onChange={(v) => setField('tokenLimitPerDay', v)}
              min={0}
              step={100}
            />
          </div>

          {/* 启用状态 */}
          <div className="mt-1 flex items-center gap-3">
            <Label htmlFor="enabled">启用状态</Label>
            <Switch id="enabled" checked={form.enabled} onCheckedChange={(v) => setField('enabled', v)} />
          </div>
        </div>

        <DialogFooter className="mt-4 gap-2">
          <Button type="button" variant="outline" className="bg-transparent" onClick={close} disabled={submitting}>
            取消
          </Button>
          <Button type="button" onClick={handleSubmit} disabled={submitting}>
            {submitting ? (mode === 'edit' ? '保存中...' : '创建中...') : mode === 'edit' ? '保存' : '创建'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

export default AddProviderKeyDialog

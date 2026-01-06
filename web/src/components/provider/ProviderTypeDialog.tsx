/**
 * ProviderTypeDialog
 * 管理端 Provider Types 增删改查对话框（按 auth_type 分行）
 */

import React, { useEffect, useMemo, useState } from 'react'
import { toast } from 'sonner'
import { api, CreateProviderTypeRequest, ProviderType, UpdateProviderTypeRequest } from '@/lib/api'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import { Textarea } from '@/components/ui/textarea'

type Mode = 'create' | 'edit'

export interface ProviderTypeDialogProps {
  open: boolean
  mode: Mode
  editing?: ProviderType | null
  onOpenChange: (open: boolean) => void
  onSaved?: () => void
}

function stringifyJson(value: any) {
  try {
    if (value === null || value === undefined) return ''
    return JSON.stringify(value, null, 2)
  } catch {
    return ''
  }
}

function parseJsonText(text: string) {
  const trimmed = text.trim()
  if (!trimmed) return null
  return JSON.parse(trimmed)
}

export const ProviderTypeDialog: React.FC<ProviderTypeDialogProps> = ({
  open,
  mode,
  editing,
  onOpenChange,
  onSaved,
}) => {
  const isEdit = mode === 'edit'
  const fieldLabelClass = 'text-sm font-medium text-neutral-700'
  const inputClass =
    'h-10 rounded-lg border-neutral-200 bg-white text-sm focus-visible:ring-2 focus-visible:ring-violet-500/40 focus-visible:ring-offset-0'
  const textareaClass =
    'rounded-lg border-neutral-200 bg-white text-sm font-mono leading-5 focus-visible:ring-2 focus-visible:ring-violet-500/40 focus-visible:ring-offset-0'
  const selectTriggerClass =
    'h-10 rounded-lg border-neutral-200 bg-white text-sm focus:ring-2 focus:ring-violet-500/40 focus:ring-offset-0'
  const sectionTitleClass = 'text-sm font-medium text-neutral-800'
  const sectionHintClass = 'text-xs text-neutral-500'

  const initial = useMemo(() => {
    const p = editing
    return {
      name: p?.name || '',
      display_name: p?.display_name || '',
      auth_type: (p?.auth_type as 'api_key' | 'oauth') || 'api_key',
      base_url: p?.base_url || '',
      is_active: p?.is_active ?? true,
      config_json: stringifyJson(p?.config_json),
      token_mappings_json: stringifyJson(p?.token_mappings_json),
      model_extraction_json: stringifyJson(p?.model_extraction_json),
      auth_configs_json: stringifyJson(p?.auth_configs_json ?? null),
    }
  }, [editing])

  const [submitting, setSubmitting] = useState(false)
  const [form, setForm] = useState(initial)

  useEffect(() => {
    if (open) setForm(initial)
  }, [open, initial])

  const setField = <K extends keyof typeof form>(key: K, value: (typeof form)[K]) => {
    setForm((prev) => ({ ...prev, [key]: value }))
  }

  const submit = async () => {
    try {
      setSubmitting(true)
      if (!form.name.trim()) {
        toast.error('请填写 name')
        return
      }
      if (!form.display_name.trim()) {
        toast.error('请填写 display_name')
        return
      }
      if (!form.base_url.trim()) {
        toast.error('请填写 base_url')
        return
      }

      let config_json: any = null
      let token_mappings_json: any = null
      let model_extraction_json: any = null
      let auth_configs_json: any = null

      try {
        config_json = parseJsonText(form.config_json)
      } catch (e: any) {
        toast.error(`config_json JSON 无效：${e?.message || '解析失败'}`)
        return
      }
      try {
        token_mappings_json = parseJsonText(form.token_mappings_json)
      } catch (e: any) {
        toast.error(`token_mappings_json JSON 无效：${e?.message || '解析失败'}`)
        return
      }
      try {
        model_extraction_json = parseJsonText(form.model_extraction_json)
      } catch (e: any) {
        toast.error(`model_extraction_json JSON 无效：${e?.message || '解析失败'}`)
        return
      }
      try {
        auth_configs_json = parseJsonText(form.auth_configs_json)
      } catch (e: any) {
        toast.error(`auth_configs_json JSON 无效：${e?.message || '解析失败'}`)
        return
      }

      if (isEdit) {
        if (!editing?.id) {
          toast.error('缺少编辑对象 ID')
          return
        }
        const payload: UpdateProviderTypeRequest = {
          name: form.name.trim(),
          display_name: form.display_name.trim(),
          base_url: form.base_url.trim(),
          is_active: form.is_active,
          config_json,
          token_mappings_json,
          model_extraction_json,
          auth_configs_json,
        }
        const res = await api.auth.updateProviderType(editing.id, payload)
        if (!res.success) {
          toast.error(res.error?.message || '更新失败')
          return
        }
        toast.success('保存成功')
      } else {
        const payload: CreateProviderTypeRequest = {
          name: form.name.trim(),
          display_name: form.display_name.trim(),
          auth_type: form.auth_type,
          base_url: form.base_url.trim(),
          is_active: form.is_active,
          config_json,
          token_mappings_json,
          model_extraction_json,
          auth_configs_json,
        }
        const res = await api.auth.createProviderType(payload)
        if (!res.success) {
          toast.error(res.error?.message || '创建失败')
          return
        }
        toast.success('创建成功')
      }

      onOpenChange(false)
      onSaved?.()
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[760px] max-h-[80vh] overflow-y-auto">
        <DialogHeader className="text-left">
          <DialogTitle>
            {isEdit ? '编辑服务商类型' : '新增服务商类型'}
          </DialogTitle>
        </DialogHeader>

        <div className="mt-2 grid grid-cols-1 gap-4">
          <div className="rounded-xl border border-neutral-200 bg-neutral-50/60 p-4">
            <div className="mb-4">
              <div className={sectionTitleClass}>基础信息</div>
              <div className={sectionHintClass}>用于识别服务商类型与认证方式</div>
            </div>
            <div className="grid grid-cols-1 gap-4 md:grid-cols-3">
              <div className="space-y-2">
                <Label className={fieldLabelClass}>
                  <span className="text-red-500 mr-1">*</span>name
                </Label>
                <Input
                  value={form.name}
                  onChange={(e) => setField('name', e.target.value)}
                  placeholder="例如 openai"
                  className={inputClass}
                />
              </div>

              <div className="space-y-2 md:col-span-2">
                <Label className={fieldLabelClass}>
                  <span className="text-red-500 mr-1">*</span>display_name
                </Label>
                <Input
                  value={form.display_name}
                  onChange={(e) => setField('display_name', e.target.value)}
                  placeholder="例如 OpenAI ChatGPT"
                  className={inputClass}
                />
              </div>
            </div>

            <div className="mt-4 grid grid-cols-1 gap-4 md:grid-cols-3">
              <div className="space-y-2">
                <Label className={fieldLabelClass}>
                  <span className="text-red-500 mr-1">*</span>auth_type
                </Label>
                <Select
                  value={form.auth_type}
                  onValueChange={(v: any) => setField('auth_type', v)}
                  disabled={isEdit}
                >
                  <SelectTrigger className={selectTriggerClass}>
                    <SelectValue placeholder="选择认证类型" />
                  </SelectTrigger>
                  <SelectContent className="rounded-lg border border-neutral-200 bg-white">
                    <SelectItem value="api_key">api_key</SelectItem>
                    <SelectItem value="oauth">oauth</SelectItem>
                  </SelectContent>
                </Select>
              </div>

              <div className="space-y-2 md:col-span-2">
                <Label className={fieldLabelClass}>
                  <span className="text-red-500 mr-1">*</span>base_url
                </Label>
                <Input
                  value={form.base_url}
                  onChange={(e) => setField('base_url', e.target.value)}
                  placeholder="例如 api.openai.com"
                  className={inputClass}
                />
              </div>
            </div>

            <div className="mt-4 flex items-center gap-3">
              <Label className={fieldLabelClass}>启用</Label>
              <Switch
                checked={form.is_active}
                onCheckedChange={(v) => setField('is_active', v)}
                className="data-[state=checked]:bg-violet-600 data-[state=unchecked]:bg-neutral-200 focus-visible:ring-violet-500/40"
              />
            </div>
          </div>

          <div className="rounded-xl border border-neutral-200 bg-white p-4">
            <div className="mb-4">
              <div className={sectionTitleClass}>通用配置</div>
              <div className={sectionHintClass}>可选，输入 JSON 对象</div>
            </div>
            <div className="space-y-4">
              <div className="space-y-2">
                <Label className={fieldLabelClass}>config_json</Label>
                <Textarea
                  value={form.config_json}
                  onChange={(e) => setField('config_json', e.target.value)}
                  rows={4}
                  placeholder="可选，JSON 对象"
                  className={textareaClass}
                />
              </div>

              <div className="space-y-2">
                <Label className={fieldLabelClass}>token_mappings_json</Label>
                <Textarea
                  value={form.token_mappings_json}
                  onChange={(e) => setField('token_mappings_json', e.target.value)}
                  rows={6}
                  placeholder="可选，JSON 对象"
                  className={textareaClass}
                />
              </div>
            </div>
          </div>

          <div className="rounded-xl border border-neutral-200 bg-white p-4">
            <div className="mb-4">
              <div className={sectionTitleClass}>模型解析</div>
              <div className={sectionHintClass}>用于提取模型名称或回退策略</div>
            </div>
            <div className="space-y-2">
              <Label className={fieldLabelClass}>model_extraction_json</Label>
              <Textarea
                value={form.model_extraction_json}
                onChange={(e) => setField('model_extraction_json', e.target.value)}
                rows={6}
                placeholder="可选，JSON 对象"
                className={textareaClass}
              />
            </div>
          </div>

          <div className="rounded-xl border border-neutral-200 bg-white p-4">
            <div className="mb-4">
              <div className={sectionTitleClass}>认证配置</div>
              <div className={sectionHintClass}>API Key 可为空对象；OAuth 需完整配置</div>
            </div>
            <div className="space-y-2">
              <Label className={fieldLabelClass}>auth_configs_json</Label>
              <Textarea
                value={form.auth_configs_json}
                onChange={(e) => setField('auth_configs_json', e.target.value)}
                rows={8}
                placeholder="API Key 行可为空对象 {}；OAuth 行为配置对象"
                className={textareaClass}
              />
            </div>
          </div>
        </div>

        <DialogFooter className="mt-4 flex gap-3 sm:justify-end">
          <Button
            type="button"
            variant="outline"
            className="flex-1 rounded-lg border-neutral-200 bg-white text-neutral-600 hover:bg-neutral-50"
            onClick={() => onOpenChange(false)}
            disabled={submitting}
          >
            取消
          </Button>
          <Button
            type="button"
            className="flex-1 rounded-lg bg-violet-600 text-white hover:bg-violet-700"
            onClick={() => void submit()}
            disabled={submitting}
          >
            {submitting ? (isEdit ? '保存中...' : '创建中...') : isEdit ? '保存' : '创建'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

export default ProviderTypeDialog

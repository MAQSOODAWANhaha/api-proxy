/**
 * OAuthHandler.tsx
 * OAuth授权处理组件 - 手动授权码输入流程
 */

import React, { useCallback, useRef, useState } from 'react'
import { ExternalLink, Shield, AlertCircle, CheckCircle2, Copy, Clipboard } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Textarea } from '@/components/ui/textarea'
import { cn } from '@/lib/utils'
import { api, OAuthAuthorizeRequest, OAuthCallbackResponse } from '@/lib/api'
import { toast } from 'sonner'
import { copyWithFeedback } from '@/lib/clipboard'

/** OAuth状态类型 */
export type OAuthStatus = 'idle' | 'authorizing' | 'waiting_code' | 'exchanging' | 'success' | 'error' | 'cancelled'

/** OAuth结果 */
export interface OAuthResult {
  success: boolean
  data?: OAuthCallbackResponse
  error?: string
  cancelled?: boolean
}

/** 组件Props */
export interface OAuthHandlerProps {
  /** OAuth请求参数 */
  request: OAuthAuthorizeRequest
  /** 当前OAuth状态 */
  status: OAuthStatus
  /** 状态变更回调 */
  onStatusChange: (status: OAuthStatus) => void
  /** OAuth完成回调 */
  onComplete: (result: OAuthResult) => void
  /** 是否禁用 */
  disabled?: boolean
  /** 自定义样式类名 */
  className?: string
  /** 按钮文本 */
  buttonText?: string
  /** 按钮变体 */
  buttonVariant?: 'default' | 'outline' | 'secondary'
}

/**
 * OAuthHandler OAuth授权处理器
 * - 启动OAuth授权流程
 * - 管理OAuth弹窗
 * - 手动授权码输入
 * - 交换访问令牌
 */
const OAuthHandler: React.FC<OAuthHandlerProps> = ({
  request,
  status,
  onStatusChange,
  onComplete,
  disabled = false,
  className,
  buttonText = '开始OAuth授权',
  buttonVariant = 'default',
}) => {
  const sessionIdRef = useRef<string | null>(null)
  const [authUrl, setAuthUrl] = useState<string>('')
  const [authCode, setAuthCode] = useState<string>('')
  const [isExchanging, setIsExchanging] = useState(false)

  /** 清理资源 */
  const cleanup = useCallback(() => {
    // 清理会话ID和状态
    sessionIdRef.current = null
    setAuthUrl('')
    setAuthCode('')
    setIsExchanging(false)
  }, [])

  /** 启动OAuth授权流程 */
  const startOAuthFlow = useCallback(async () => {
    if (status !== 'idle' || disabled) return

    try {
      onStatusChange('authorizing')
      
      // 调用后端API启动OAuth流程
      const response = await api.auth.initiateOAuth(request)
      
      if (!response.success || !response.data) {
        throw new Error(response.error?.message || 'OAuth授权启动失败')
      }

      const { authorize_url, session_id } = response.data
      
      if (!authorize_url || !authorize_url.trim()) {
        throw new Error('获取授权URL失败，授权URL为空')
      }
      
      sessionIdRef.current = session_id
      setAuthUrl(authorize_url)
      onStatusChange('waiting_code')
      
      toast.info('请在新打开的页面中完成授权，然后复制授权码回来')
      
      // 在新标签页中打开授权页面
      const popup = window.open(authorize_url, '_blank', 'noopener,noreferrer')
      if (popup) {
        popup.opener = null
      }
      
      if (!popup || popup.closed) {
        toast.warning('无法打开弹窗，请检查浏览器弹窗设置，或手动复制下方链接打开')
      }

    } catch (error) {
      cleanup()
      onStatusChange('error')
      const errorMessage = error instanceof Error ? error.message : 'OAuth授权启动失败'
      onComplete({
        success: false,
        error: errorMessage,
      })
      toast.error(errorMessage)
    }
  }, [request, status, disabled, onStatusChange, onComplete, cleanup])

  /** 提交授权码 */
  const submitAuthCode = useCallback(async () => {
    if (!authCode.trim() || !sessionIdRef.current || isExchanging) return

    try {
      setIsExchanging(true)
      onStatusChange('exchanging')
      
      // 调用后端API交换token
      const response = await api.auth.exchangeOAuthToken({
        session_id: sessionIdRef.current,
        authorization_code: authCode.trim(),
      })
      
      if (!response.success || !response.data) {
        throw new Error(response.error?.message || 'Token交换失败')
      }

      cleanup()
      onStatusChange('success')
      onComplete({
        success: true,
        data: response.data,
      })
      toast.success('OAuth授权成功！')

    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Token交换失败'
      onStatusChange('error')
      onComplete({
        success: false,
        error: errorMessage,
      })
      toast.error(errorMessage)
    } finally {
      setIsExchanging(false)
    }
  }, [authCode, isExchanging, onStatusChange, onComplete, cleanup])

  /** 复制授权URL */
  const copyAuthUrl = useCallback(async () => {
    if (!authUrl) return

    await copyWithFeedback(authUrl, '授权链接')
  }, [authUrl])

  /** 取消OAuth流程 */
  const cancelOAuthFlow = useCallback(() => {
    cleanup()
    onStatusChange('cancelled')
    onComplete({
      success: false,
      cancelled: true,
    })
    toast.info('OAuth授权已取消')
  }, [cleanup, onStatusChange, onComplete])

  /** 重新开始OAuth流程 */
  const retryOAuthFlow = useCallback(() => {
    cleanup()
    onStatusChange('idle')
  }, [cleanup, onStatusChange])

  // 组件卸载时清理资源
  React.useEffect(() => {
    return cleanup
  }, [cleanup])

  /** 渲染状态指示器 */
  const renderStatusIndicator = () => {
    switch (status) {
      case 'authorizing':
        return (
          <Badge variant="secondary" className="flex items-center gap-1">
            <div className="w-2 h-2 bg-blue-500 rounded-full animate-pulse" />
            正在启动授权...
          </Badge>
        )
      
      case 'waiting_code':
        return (
          <Badge variant="secondary" className="flex items-center gap-1">
            <div className="w-2 h-2 bg-yellow-500 rounded-full animate-pulse" />
            等待用户授权
          </Badge>
        )
      
      case 'success':
        return (
          <Badge variant="default" className="flex items-center gap-1 bg-green-500">
            <CheckCircle2 className="h-3 w-3" />
            授权成功
          </Badge>
        )
      
      case 'error':
        return (
          <Badge variant="destructive" className="flex items-center gap-1">
            <AlertCircle className="h-3 w-3" />
            授权失败
          </Badge>
        )
      
      case 'cancelled':
        return (
          <Badge variant="outline" className="flex items-center gap-1">
            <AlertCircle className="h-3 w-3" />
            授权取消
          </Badge>
        )
      
      default:
        return null
    }
  }

  return (
    <Card className={cn('', className)}>
      <CardHeader className="pb-4">
        <CardTitle className="flex items-center gap-2 text-base">
          <Shield className="h-5 w-5" />
          OAuth 2.0 授权
          {renderStatusIndicator()}
        </CardTitle>
      </CardHeader>
      
      <CardContent className="space-y-4">
        <div className="text-sm text-muted-foreground">
          <p>将打开新页面进行OAuth授权，完成授权后请复制授权码并在下方输入。</p>
          {status === 'waiting_code' && (
            <p className="mt-2 text-blue-600 dark:text-blue-400">
              💡 授权页面已打开，完成授权后请复制Authorization Code
            </p>
          )}
        </div>

        {/* 授权URL显示和复制 */}
        {authUrl && status === 'waiting_code' && (
          <div className="space-y-2">
            <Label htmlFor="auth-url">授权链接</Label>
            <div className="flex gap-2">
              <Input
                id="auth-url"
                value={authUrl}
                readOnly
                className="font-mono text-xs"
              />
              <Button
                type="button"
                size="sm"
                variant="outline"
                onClick={copyAuthUrl}
                aria-label="复制授权链接"
              >
                <Copy className="h-4 w-4" />
              </Button>
            </div>
            <p className="text-xs text-muted-foreground">
              如果页面没有自动打开，请点击复制按钮后在浏览器中打开此链接
            </p>
          </div>
        )}

        {/* 授权码输入区域 */}
        {status === 'waiting_code' && (
          <div className="space-y-2">
            <Label htmlFor="auth-code">授权码 (Authorization Code)</Label>
            <Textarea
              id="auth-code"
              placeholder="请粘贴从授权页面获取的Authorization Code..."
              value={authCode}
              onChange={(e) => setAuthCode(e.target.value)}
              className="font-mono text-xs min-h-[80px]"
              disabled={isExchanging}
            />
            <p className="text-xs text-muted-foreground">
              💡 完成授权后，将显示一个很长的授权码，请复制完整的授权码到此处
            </p>
          </div>
        )}

        <div className="flex gap-2">
          {status === 'idle' || status === 'error' || status === 'cancelled' ? (
            <Button
              onClick={status === 'idle' ? startOAuthFlow : retryOAuthFlow}
              disabled={disabled}
              variant={buttonVariant}
              className="flex items-center gap-2"
            >
              <ExternalLink className="h-4 w-4" />
              {status === 'idle' ? buttonText : '重新授权'}
            </Button>
          ) : status === 'waiting_code' ? (
            <>
              <Button
                onClick={submitAuthCode}
                disabled={!authCode.trim() || isExchanging}
                className="flex items-center gap-2"
              >
                <Clipboard className="h-4 w-4" />
                {isExchanging ? '交换Token中...' : '提交授权码'}
              </Button>
              <Button
                onClick={cancelOAuthFlow}
                variant="outline"
                className="flex items-center gap-2"
                disabled={isExchanging}
              >
                <AlertCircle className="h-4 w-4" />
                取消授权
              </Button>
            </>
          ) : null}
        </div>

        {/* 操作说明 */}
        {status === 'waiting_code' && (
          <div className="text-xs text-blue-600 dark:text-blue-400 bg-blue-50 dark:bg-blue-950/20 p-3 rounded-md">
            <strong>操作步骤：</strong>
            <ol className="mt-1 space-y-1 list-decimal list-inside">
              <li>在新打开的授权页面中完成登录和授权</li>
              <li>授权完成后会显示一个长的Authorization Code</li>
              <li>复制完整的Authorization Code到上方输入框</li>
              <li>点击"提交授权码"完成OAuth流程</li>
            </ol>
          </div>
        )}

        {/* 安全提示 */}
        <div className="text-xs text-muted-foreground bg-muted/50 p-3 rounded-md">
          🔐 <strong>安全提示：</strong>
          OAuth授权过程中不会要求您输入密码到我们的系统，
          所有授权操作都在官方授权服务器上完成，确保您的账户安全。
        </div>
      </CardContent>
    </Card>
  )
}

export default OAuthHandler

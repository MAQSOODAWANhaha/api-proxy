/**
 * OAuthCallbackPage.tsx
 * 显示和复制OAuth回调URL中的code参数
 */
import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import { Copy, ExternalLink, ArrowLeft } from 'lucide-react'
import { copyWithFeedback } from '@/lib/clipboard'

export default function OAuthCallbackPage() {
  const navigate = useNavigate()
  const [code, setCode] = useState<string>('')
  const [state, setState] = useState<string>('')
  const [scope, setScope] = useState<string>('')
  const [callbackUrl, setCallbackUrl] = useState<string>('')

  useEffect(() => {
    // 获取URL参数
    const urlParams = new URLSearchParams(window.location.search)
    const codeParam = urlParams.get('code')
    const stateParam = urlParams.get('state')
    const scopeParam = urlParams.get('scope')

    if (codeParam) {
      setCode(codeParam)
      setState(stateParam || '')
      setScope(scopeParam || '')
      setCallbackUrl(window.location.href)
    }
  }, [])

  const copyToClipboard = async (text: string, type: string) => {
    await copyWithFeedback(text, type)
  }

  const formatScopes = (scopeString: string) => {
    if (!scopeString) return []
    return scopeString.split('+')
  }

  const handleBack = () => {
    navigate('/api')
  }

  return (
    <div className="container mx-auto p-6 max-w-4xl">
      <div className="mb-6">
        <Button
          variant="outline"
          onClick={handleBack}
          className="mb-4"
        >
          <ArrowLeft className="h-4 w-4 mr-2" />
          返回API管理
        </Button>
      </div>

      <div className="space-y-6">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <ExternalLink className="h-5 w-5" />
              OAuth回调信息
            </CardTitle>
            <CardDescription>
              从OAuth回调URL中提取的授权信息
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-6">
            {/* Code字段 */}
            <div>
              <label className="text-sm font-medium mb-2 block">Authorization Code</label>
              <div className="flex gap-2">
                <Input
                  value={code}
                  readOnly
                  className="font-mono text-sm"
                  placeholder="No code found in URL"
                />
                <Button
                  variant="outline"
                  size="icon"
                  onClick={() => copyToClipboard(code, 'Authorization Code')}
                  disabled={!code}
                  aria-label="复制 Authorization Code"
                >
                  <Copy className="h-4 w-4" />
                </Button>
              </div>
              {code && (
                <p className="text-xs text-muted-foreground mt-1">
                  这是用于获取access_token的临时授权码，有效期通常为10分钟
                </p>
              )}
            </div>

            {/* State字段 */}
            {state && (
              <div>
                <label className="text-sm font-medium mb-2 block">State</label>
                <div className="flex gap-2">
                  <Input
                    value={state}
                    readOnly
                    className="font-mono text-sm"
                  />
                  <Button
                    variant="outline"
                    size="icon"
                    onClick={() => copyToClipboard(state, 'State')}
                    aria-label="复制 State"
                  >
                    <Copy className="h-4 w-4" />
                  </Button>
                </div>
                <p className="text-xs text-muted-foreground mt-1">
                  用于防止CSRF攻击的随机字符串
                </p>
              </div>
            )}

            {/* Scope字段 */}
            {scope && (
              <div>
                <label className="text-sm font-medium mb-2 block">Scopes</label>
                <div className="flex flex-wrap gap-2">
                  {formatScopes(scope).map((scopeItem, index) => (
                    <Badge key={index} variant="secondary">
                      {scopeItem}
                    </Badge>
                  ))}
                </div>
                <p className="text-xs text-muted-foreground mt-1">
                  请求的权限范围
                </p>
              </div>
            )}

            {/* 完整URL */}
            <div>
              <label className="text-sm font-medium mb-2 block">完整回调URL</label>
              <div className="flex gap-2">
                <Input
                  value={callbackUrl}
                  readOnly
                  className="font-mono text-xs"
                  placeholder="No callback URL"
                />
                <Button
                  variant="outline"
                  size="icon"
                  onClick={() => copyToClipboard(callbackUrl, '回调URL')}
                  disabled={!callbackUrl}
                  aria-label="复制回调URL"
                >
                  <Copy className="h-4 w-4" />
                </Button>
              </div>
            </div>
          </CardContent>
        </Card>

        {/* 使用说明 */}
        <Card>
          <CardHeader>
            <CardTitle>使用说明</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="text-sm space-y-3">
              <p>
                <strong>1. 复制Code：</strong>点击Code字段右侧的复制按钮，将授权码复制到剪贴板
              </p>
              <p>
                <strong>2. 获取Access Token：</strong>使用复制的code向token端点请求access_token
              </p>
              <p>
                <strong>3. 注意时效：</strong>Authorization Code通常只有10分钟的有效期，请尽快使用
              </p>
              <p>
                <strong>4. 安全性：</strong>请勿在客户端直接处理code，最佳实践是通过后端服务完成token交换
              </p>
            </div>
          </CardContent>
        </Card>

        {!code && (
          <Card className="border-yellow-200 bg-yellow-50">
            <CardHeader>
              <CardTitle className="text-yellow-800">未找到Code参数</CardTitle>
            </CardHeader>
            <CardContent>
              <p className="text-yellow-700">
                当前URL中没有找到code参数。请确保通过正确的OAuth回调链接访问此页面。
                格式应为：<code className="bg-yellow-100 px-1 rounded">.../auth/callback?code=YOUR_CODE&state=YOUR_STATE</code>
              </p>
            </CardContent>
          </Card>
        )}
      </div>
    </div>
  )
}

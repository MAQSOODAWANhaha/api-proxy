我已用 GitHub 工具读完该仓库的认证与请求构造代码与文档，并针对三种场景梳理了 URL、Header、Query、Body 的差异与来源。

结论速览

OAuth（无/有 project_id）都通过 Code Assist 后端调用，URL 走 cloudcode-pa，Header 用 OAuth Bearer，Body 里是否带 project 取决于是否已确定 project_id。
OAuth 无 project_id 时，先用 loadCodeAssist/onboardUser 完成“托管项目”分配；生成请求前会拿到一个实际 project_id，再写入后续请求 Body。
Gemini API Key 模式走 Google AI Studio 公共端点（generativelanguage），Header 用 x-goog-api-key（由 @google/genai 处理），Body 不包含 project 字段。
三种场景详解

1）OAuth 登录（无 project_id）

URL
统一走 Code Assist 后端：https://cloudcode-pa.googleapis.com/v1internal:{method}（如 generateContent、countTokens、streamGenerateContent）
参考：packages/core/src/code_assist/server.ts
Header
Authorization: Bearer <access_token>（由 OAuth2Client 自动注入）
Content-Type: application/json
User-Agent: GeminiCLI/<version> (...)（统一设置）
参考：packages/core/src/core/contentGenerator.ts, packages/core/src/code_assist/server.ts
Query
仅流式使用 alt=sse
参考：packages/core/src/code_assist/server.ts
Body
初始化与开通阶段：
loadCodeAssist 请求体中 cloudaicompanionProject 为空（未指定），如需开通则调用 onboardUser，免费层明确“不要”携带 project 字段
成功后服务端会返回托管的 cloudaicompanionProject.id
参考：packages/core/src/code_assist/setup.ts, packages/core/src/code_assist/server.ts
生成与计数阶段：
生成请求体结构为 Code Assist 适配层的包装：{ model, project?, user_prompt_id, request:{ contents, systemInstruction, tools, generationConfig, ... } }
一旦完成上一步，project 会被设置为服务端分配的实际 project_id；生成请求不会带空的 project
参考：packages/core/src/code_assist/converter.ts
2）OAuth 登录（有 project_id）

URL
同上，走 cloudcode-pa.googleapis.com/v1internal:{method}
Header
同上，Authorization: Bearer <access_token> + Content-Type + User-Agent
Query
流式 alt=sse；其他无额外 Query
Body
初始化与开通阶段：
loadCodeAssist 与（必要时的）onboardUser 的请求体都带 cloudaicompanionProject=<env project_id>
生成与计数阶段：
project 明确为 <env project_id>
参考：packages/core/src/code_assist/setup.ts, packages/core/src/code_assist/converter.ts
3）Gemini API Key 认证（GEMINI_API_KEY）

URL
使用 @google/genai 客户端直连 Gemini API 公共端点，典型为 https://generativelanguage.googleapis.com（具体版本与路由由库封装，如 v1/v1beta 的 /models/{model}:generateContent 或 :streamGenerateContent）
参考（入口与实例化）：packages/core/src/core/contentGenerator.ts
Header
x-goog-api-key: <GEMINI_API_KEY>（由 @google/genai 注入）
Content-Type: application/json
User-Agent: GeminiCLI/<version> (...)
若启用使用统计，还会附加 x-gemini-api-privileged-user-id: <installationId>
参考：packages/core/src/core/contentGenerator.ts
Query
流式时通常通过 :streamGenerateContent（由 @google/genai 处理），底层多采用 alt=sse 参数，但细节由库封装
Body
直接使用 Gemini API 的请求结构（contents, systemInstruction, tools, generationConfig, ...）；不包含 project
参考（请求映射同源）：packages/core/src/code_assist/converter.ts 中 Vertex 风格字段与 @google/genai 的字段一一对应；API Key 路径下不含 project
证据与代码位置

统一创建与分流（OAuth/Key/Vertex）：packages/core/src/core/contentGenerator.ts
OAuth 具体实现（OAuth2 获取与回调）：packages/core/src/code_assist/oauth2.ts
Code Assist 服务端封装（URL、Header、SSE）：packages/core/src/code_assist/server.ts
Code Assist 请求/响应映射（Body 结构与 project 字段）：packages/core/src/code_assist/converter.ts
OAuth 初始化与 project 选择/开通（无/有 project_id 路径差异）：packages/core/src/code_assist/setup.ts
CLI 认证方式校验（环境变量判定）：packages/cli/src/config/auth.ts
文档（三种认证方式）：README.md, docs/cli/authentication.md
差异要点归纳

URL
OAuth → cloudcode-pa.googleapis.com/v1internal:{method}
API Key → generativelanguage.googleapis.com（由 @google/genai 决定具体路由）
Header
OAuth → Authorization: Bearer <token>
API Key → x-goog-api-key: <GEMINI_API_KEY>
共通 → Content-Type: application/json, User-Agent: GeminiCLI/...
统计可选 → x-gemini-api-privileged-user-id（API Key/Vertex 路径）
Query
仅流式统一使用 alt=sse（OAuth 明确可见；API Key 由库封装）
Body
OAuth → Code Assist 包装体，含 project（无 project_id 时先经 loadCodeAssist/onboardUser 获取；有 project_id 则直接使用）
API Key → 标准 Gemini API 体，不含 project
如果你需要，我可以继续：

把这三种场景的实际 HTTP 样例（curl 或等价 JSON）按“生成/流式/计数”各给一份；
进一步对比 Vertex AI（GOOGLE_API_KEY + GOOGLE_GENAI_USE_VERTEXAI=true）的 URL/Header/Body 差异。
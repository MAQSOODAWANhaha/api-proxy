1：路由匹配
    /v1internal\\:loadCodeAssist
    /v1beta/models/{model}:loadCodeAssist 
    修改body:
    如果选择的key有配置project_id metadata.duetProject=project_id
    如果选择的key有配置project_id body.cloudaicompanionProject=project_id
2: 路由匹配
    /v1internal\\:onboardUser
    /v1beta/models/{model}:onboardUser
    修改body:
    如果选择的key有配置project_id body.cloudaicompanionProject=project_id
3: 路由匹配
    /v1internal\\:generateContent
    /v1beta/models/{model}:generateContent
    修改body:
    如果选择的key有配置project_id body.project=project_id
4: 路由匹配
    /v1internal\\:streamGenerateContent
    /v1beta/models/{model}:streamGenerateContent
    修改body:
    如果选择的key有配置project_id body.project=project_id
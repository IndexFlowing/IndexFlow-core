-- 为 sites 表增加 Cloudflare 防火墙放行密钥 / 自定义 User-Agent 字段
ALTER TABLE sites ADD COLUMN custom_user_agent TEXT;
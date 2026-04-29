package config

import "time"

// LLMConfig LLM API 配置
type LLMConfig struct {
	Provider string        `yaml:"provider"` // openai | anthropic | custom
	Endpoint string        `yaml:"endpoint"` // API 端点
	APIKey   string        `yaml:"api_key"`  // API key（留空从环境变量 LLM_API_KEY 读取）
	Model    string        `yaml:"model"`    // 模型名
	Timeout  time.Duration `yaml:"timeout"`  // 请求超时
}

// DefaultLLMConfig 返回默认 LLM 配置
func DefaultLLMConfig() LLMConfig {
	return LLMConfig{
		Provider: "openai",
		Endpoint: "https://api.openai.com/v1",
		APIKey:   "",
		Model:    "gpt-4o",
		Timeout:  60 * time.Second,
	}
}

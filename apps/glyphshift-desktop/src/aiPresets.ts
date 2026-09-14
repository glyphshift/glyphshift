import type { AiProviderProtocol, AiReasoningEffort } from './useAiTranslation'

interface AiPreset {
  id: string
  documentationUrl: string
  name: string
  protocol: AiProviderProtocol
  baseUrl: string
  modelId: string
  reasoningEffort: AiReasoningEffort
}

// Service defaults are editable drafts, not persisted provider identities.
export const aiPresets: readonly AiPreset[] = [
  { id: 'deepseek', documentationUrl: 'https://api-docs.deepseek.com/zh-cn/', name: 'DeepSeek', protocol: 'open_ai_chat_completions', baseUrl: 'https://api.deepseek.com', modelId: '', reasoningEffort: 'disabled' },
  { id: 'qwen', documentationUrl: 'https://help.aliyun.com/zh/model-studio/get-api-key/', name: 'Qwen', protocol: 'open_ai_chat_completions', baseUrl: 'https://dashscope.aliyuncs.com/compatible-mode/v1', modelId: '', reasoningEffort: 'automatic' },
  { id: 'siliconflow', documentationUrl: 'https://docs.siliconflow.cn/docs/userguide/quickstart', name: 'SiliconFlow', protocol: 'open_ai_chat_completions', baseUrl: 'https://api.siliconflow.cn/v1', modelId: '', reasoningEffort: 'automatic' },
  { id: 'openai', documentationUrl: 'https://platform.openai.com/docs/api-reference/models', name: 'OpenAI', protocol: 'open_ai_responses', baseUrl: 'https://api.openai.com/v1', modelId: '', reasoningEffort: 'automatic' },
  { id: 'anthropic', documentationUrl: 'https://platform.claude.com/docs/en/api/models', name: 'Claude', protocol: 'anthropic_messages', baseUrl: 'https://api.anthropic.com', modelId: '', reasoningEffort: 'automatic' },
  { id: 'gemini', documentationUrl: 'https://ai.google.dev/api/models', name: 'Gemini', protocol: 'gemini_generate_content', baseUrl: 'https://generativelanguage.googleapis.com/v1beta', modelId: '', reasoningEffort: 'automatic' },
  { id: 'openrouter', documentationUrl: 'https://openrouter.ai/docs/quickstart', name: 'OpenRouter', protocol: 'open_ai_compatible', baseUrl: 'https://openrouter.ai/api/v1', modelId: '', reasoningEffort: 'automatic' },
  { id: 'groq', documentationUrl: 'https://console.groq.com/docs/overview', name: 'Groq', protocol: 'open_ai_chat_completions', baseUrl: 'https://api.groq.com/openai/v1', modelId: '', reasoningEffort: 'automatic' },
  { id: 'mistral', documentationUrl: 'https://docs.mistral.ai/api/endpoint/models', name: 'Mistral', protocol: 'open_ai_chat_completions', baseUrl: 'https://api.mistral.ai/v1', modelId: '', reasoningEffort: 'automatic' },
  { id: 'xai', documentationUrl: 'https://docs.x.ai/developers/rest-api-reference/inference/models', name: 'xAI', protocol: 'open_ai_chat_completions', baseUrl: 'https://api.x.ai/v1', modelId: '', reasoningEffort: 'automatic' },
  { id: 'ollama', documentationUrl: 'https://docs.ollama.com/api/tags', name: 'Ollama', protocol: 'ollama_chat', baseUrl: 'http://127.0.0.1:11434/api', modelId: '', reasoningEffort: 'automatic' },
]

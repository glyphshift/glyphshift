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
  { id: 'deepseek', documentationUrl: 'https://api-docs.deepseek.com/zh-cn/', name: 'DeepSeek', protocol: 'open_ai_chat_completions', baseUrl: 'https://api.deepseek.com', modelId: 'deepseek-v4-flash', reasoningEffort: 'disabled' },
  { id: 'qwen', documentationUrl: 'https://help.aliyun.com/zh/model-studio/get-api-key/', name: 'Qwen', protocol: 'open_ai_chat_completions', baseUrl: 'https://dashscope.aliyuncs.com/compatible-mode/v1', modelId: 'qwen-plus', reasoningEffort: 'automatic' },
  { id: 'siliconflow', documentationUrl: 'https://docs.siliconflow.cn/docs/userguide/quickstart', name: 'SiliconFlow', protocol: 'open_ai_chat_completions', baseUrl: 'https://api.siliconflow.cn/v1', modelId: 'Qwen/Qwen2.5-72B-Instruct', reasoningEffort: 'automatic' },
]

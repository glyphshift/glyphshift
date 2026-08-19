import { expect, test } from '@playwright/test'
import { existsSync, readFileSync } from 'node:fs'
import { join } from 'node:path'

const repositoryRoot = join(import.meta.dirname, '..', '..', '..')

test('bilingual READMEs lead with user problems and product use instead of repository internals', () => {
  const chinese = readFileSync(join(repositoryRoot, 'README.md'), 'utf8')
  const english = readFileSync(join(repositoryRoot, 'README.en.md'), 'utf8')

  expect(chinese).toContain('[English](README.en.md)')
  expect(english).toContain('[简体中文](README.md)')
  const chinesePreviews = [...chinese.matchAll(/src="(preview\/[^"]+)"/g)].map(match => match[1])
  const englishPreviews = [...english.matchAll(/src="(preview\/[^"]+)"/g)].map(match => match[1])
  expect(chinesePreviews).toEqual(englishPreviews)
  for (const preview of chinesePreviews) expect(existsSync(join(repositoryRoot, preview))).toBe(true)
  expect(chinese).not.toContain('src="preview/探针.png"')
  expect(english).not.toContain('src="preview/探针.png"')

  for (const heading of [
    '## Glyphshift 解决什么问题',
    '## 它如何工作',
    '## 核心能力',
    '## 开始使用',
    '## AI 翻译与隐私',
    '## 适用范围与边界',
  ]) expect(chinese).toContain(heading)

  for (const heading of [
    '## What problem does Glyphshift solve?',
    '## How it works',
    '## Core capabilities',
    '## Get started',
    '## AI translation and privacy',
    '## Scope and limitations',
  ]) expect(english).toContain(heading)

  expect(chinese).not.toContain('## 仓库结构')
  expect(chinese).not.toContain('## 开发验证')
  expect(chinese).not.toContain('cargo clippy')
  expect(english).not.toContain('## Repository structure')
  expect(english).not.toContain('## Development validation')
  expect(chinese).toContain('任意源语言和目标语言')
  expect(chinese).toContain('不会限制词典可以翻译的语言')
  expect(chinese).not.toContain('让没有中文')
  expect(english).toContain('any source and target language pair')
  expect(english).toContain('do not restrict the languages a dictionary can translate')
})

test('version tags build the verified Windows installer and publish it to the current repository', () => {
  const workflow = readFileSync(join(repositoryRoot, '.github', 'workflows', 'release.yml'), 'utf8')

  expect(workflow).toContain("tags:\n      - 'v*'")
  expect(workflow).toContain('runs-on: windows-latest')
  expect(workflow).toContain('contents: write')
  expect(workflow).toContain('./scripts/build-desktop-release.ps1')
  expect(workflow).toContain('gh release create $tag @assets')
  expect(workflow).toContain('--verify-tag')
  expect(workflow).toContain('candidate-manifest.json')
  expect(workflow).toContain('SHA256SUMS.txt')
  expect(workflow).not.toMatch(/OWNER\/REPO|repository:\s+[^\n]+|PERSONAL_ACCESS_TOKEN|\bPAT\b/)
})

test('desktop copy uses novice task language and keeps implementation terms out of routine surfaces', () => {
  const chinese = readFileSync(join(repositoryRoot, 'apps', 'glyphshift-desktop', 'src', 'locales', 'zh-CN.ts'), 'utf8')
  const english = readFileSync(join(repositoryRoot, 'apps', 'glyphshift-desktop', 'src', 'locales', 'en-US.ts'), 'utf8')
  const aiPanel = readFileSync(join(repositoryRoot, 'apps', 'glyphshift-desktop', 'src', 'components', 'AiProfilesPanel.vue'), 'utf8')
  const diagnostics = readFileSync(join(repositoryRoot, 'apps', 'glyphshift-desktop', 'src', 'components', 'RuntimeDiagnosticsModal.vue'), 'utf8')
  const routineSurfaces = [
    'SettingsView.vue',
    'SoftwareTable.vue',
    'DictionaryLibrary.vue',
    'WorkflowTable.vue',
    'CaptureView.vue',
    'TranslationTasksView.vue',
  ].map(file => readFileSync(join(repositoryRoot, 'apps', 'glyphshift-desktop', 'src', 'components', file), 'utf8')).join('\n')
  const chineseValues = [...chinese.matchAll(/:\s*'([^']*)'/g)].map(match => match[1]).join('\n')
  const englishValues = [...english.matchAll(/:\s*'([^']*)'/g)].map(match => match[1]).join('\n')

  for (const jargon of ['AI Profile', 'Provider', 'Runtime Bundle', 'Profile 文件', '本地修订']) {
    expect(chineseValues).not.toContain(jargon)
  }
  for (const jargon of ['AI profile', 'Provider', 'Runtime Bundle', 'Profile files', 'Local revision']) {
    expect(englishValues).not.toContain(jargon)
  }
  expect(chineseValues.match(/Adapter/g) ?? []).toHaveLength(1)
  expect(englishValues.match(/\(adapter\)/g) ?? []).toHaveLength(1)
  expect(chinese).toContain("profilesTitle: 'AI 配置'")
  expect(english).toContain("profilesTitle: 'AI connections'")
  for (const redundant of [
    '这些偏好会立即应用，并保存在当前设备上',
    '设置翻译前确认，以及翻译时要使用的 AI 服务和模型',
    '控制 Windows 登录启动、关闭窗口行为',
    '为每个软件组合兼容方式、有序词典和字体设置',
  ]) expect(chineseValues).not.toContain(redundant)
  expect(aiPanel).not.toContain('credentialStorageHint')
  expect(aiPanel).not.toContain('modelDiscoveryNotTested')
  expect(diagnostics).not.toContain('publicationIdentity')
  expect(routineSurfaces).toContain('icon="i-tabler-list-check"')
  expect(routineSurfaces).not.toContain('<UtilityPageShell\n    title-id="translation-tasks-title"')
  for (const redundantKey of [
    "t('settings.description')",
    "t('settings.appearanceDescription')",
    "t('software.description')",
    "t('dictionaries.description')",
    "t('workflows.description')",
    "t('capture.description')",
    "t('ai.tasks.description')",
  ]) expect(routineSurfaces).not.toContain(redundantKey)
})

test('root documents separate domain language, product truth, and the implemented design system', () => {
  const agents = readFileSync(join(repositoryRoot, 'AGENTS.md'), 'utf8')
  const context = readFileSync(join(repositoryRoot, 'CONTEXT.md'), 'utf8')
  const product = readFileSync(join(repositoryRoot, 'PRODUCT.md'), 'utf8')
  const design = readFileSync(join(repositoryRoot, 'DESIGN.md'), 'utf8')

  expect(agents).toContain('User-approved release screenshots may live under `preview/`')
  expect(agents).toContain('after a privacy review')

  expect(context).toContain('## 用户资产')
  expect(context).toContain('## 探针')
  expect(context).toContain('## Runtime')
  expect(context).not.toContain('## 不变量')
  expect(context).not.toContain('SHA-256')
  expect(context).not.toContain('local-test/')
  expect(context).not.toMatch(/Dictionary `\/\d+`|Workflow `\/\d+`/)

  for (const heading of [
    '## 产品定位',
    '## 用户',
    '## 用户成功路径',
    '## 产品表面',
    '## 产品承诺',
    '## 适用范围与边界',
  ]) expect(product).toContain(heading)
  expect(product).toContain('设置页包含外观、AI 翻译、快捷键、应用与权限四个分区')
  expect(product).toContain('收集原文 → 填写译文 → 验证效果 → 启用工作流')
  expect(product).not.toContain('SHA-256')
  expect(product).not.toContain('ownership ledger')
  expect(product).not.toContain('跨 IPC')

  const canonicalHeadings = [
    '## Overview',
    '## Colors',
    '## Typography',
    '## Layout',
    '## Elevation & Depth',
    '## Shapes',
    '## Components',
    "## Do's and Don'ts",
  ]
  const positions = canonicalHeadings.map(heading => design.indexOf(heading))
  expect(positions.every(position => position >= 0)).toBe(true)
  expect(positions).toEqual([...positions].sort((left, right) => left - right))
  expect(design).toContain('Windows Translation Workbench')
  expect(design).toContain('Settings 只有四个大分区')
  expect(design).toContain('使用指南 / AI 翻译 / 故障排查 / 技术与兼容')
  expect(design).not.toContain('AI 翻译执行')
})

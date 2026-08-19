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

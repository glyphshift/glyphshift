import { expect, test } from '@playwright/test'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

const repositoryRoot = join(import.meta.dirname, '..', '..', '..')

test('bilingual READMEs lead with user problems and product use instead of repository internals', () => {
  const chinese = readFileSync(join(repositoryRoot, 'README.md'), 'utf8')
  const english = readFileSync(join(repositoryRoot, 'README.en.md'), 'utf8')

  expect(chinese).toContain('[English](README.en.md)')
  expect(english).toContain('[简体中文](README.md)')

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

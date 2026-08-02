import { expect, test } from '@playwright/test'
import { readdirSync, readFileSync } from 'node:fs'
import { basename, join, relative } from 'node:path'

const sourceRoot = join(import.meta.dirname, '..', 'src')
const packageRoot = join(sourceRoot, '..')

function vueSources(directory = sourceRoot): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const path = join(directory, entry.name)
    if (entry.isDirectory()) return vueSources(path)
    return entry.name.endsWith('.vue') ? [path] : []
  })
}

test('Vue surfaces use Nuxt UI for controls, tables, and overlays', () => {
  const nativeSurfacePattern = /<(button|input|select|textarea|table|dialog)\b|role=["']dialog["']/gi
  const violations = vueSources().flatMap(path => {
    const source = readFileSync(path, 'utf8')
    const nativeSurfaces = [...source.matchAll(nativeSurfacePattern)].map(match => ({
      file: relative(sourceRoot, path).replaceAll('\\', '/'),
      primitive: match[0],
    }))
    if (source.includes('@tabler/icons-vue')) {
      nativeSurfaces.push({
        file: relative(sourceRoot, path).replaceAll('\\', '/'),
        primitive: '@tabler/icons-vue',
      })
    }
    if (source.includes('btn-primary-contained')) {
      nativeSurfaces.push({
        file: relative(sourceRoot, path).replaceAll('\\', '/'),
        primitive: 'manual primary-button class',
      })
    }
    return nativeSurfaces
  })

  expect(violations, 'Use Nuxt UI primitives instead of native interactive/table/dialog surfaces.').toEqual([])

  const packageJson = JSON.parse(readFileSync(join(packageRoot, 'package.json'), 'utf8')) as {
    dependencies?: Record<string, string>
  }
  expect(packageJson.dependencies).not.toHaveProperty('@tabler/icons-vue')
})

test('management pages share the project management-page modules', () => {
  const pages = ['WorkflowTable.vue', 'SoftwareTable.vue', 'DictionaryLibrary.vue', 'FontProfileLibrary.vue']

  for (const page of pages) {
    const source = readFileSync(join(sourceRoot, 'components', page), 'utf8')
    expect(source, `${basename(page)} must use the shared page header.`).toContain('<ManagementPageHeader')
    expect(source, `${basename(page)} must use the shared table frame.`).toContain('<ManagementTableFrame')
  }

  const dictionaryEditor = readFileSync(join(sourceRoot, 'components', 'DictionaryProof.vue'), 'utf8')
  expect(dictionaryEditor, 'DictionaryProof.vue must use the shared table frame.').toContain('<ManagementTableFrame')
})

test('modal layers stay above sticky tables and transient menus', () => {
  const viteConfig = readFileSync(join(packageRoot, 'vite.config.ts'), 'utf8')
  expect(viteConfig).toContain("overlay: 'z-[80]")
  expect(viteConfig).toContain("content: 'z-[81]")
})

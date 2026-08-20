import { expect, test } from '@playwright/test'
import { existsSync, readdirSync, readFileSync } from 'node:fs'
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
  const pages = ['WorkflowTable.vue', 'SoftwareTable.vue', 'DictionaryLibrary.vue', 'CaptureView.vue']
  const pageHeaderPages = [...pages, 'TranslationTasksView.vue']

  for (const page of pageHeaderPages) {
    const source = readFileSync(join(sourceRoot, 'components', page), 'utf8')
    expect(source, `${basename(page)} must use the shared page header.`).toContain('<ManagementPageHeader')
  }

  for (const page of pages) {
    const source = readFileSync(join(sourceRoot, 'components', page), 'utf8')
    expect(source, `${basename(page)} must use the shared table frame.`).toContain('<ManagementTableFrame')
  }

  const dictionaryEditor = readFileSync(join(sourceRoot, 'components', 'DictionaryProof.vue'), 'utf8')
  expect(dictionaryEditor, 'DictionaryProof.vue must use the shared table frame.').toContain('<ManagementTableFrame')

  const detailPages = ['WorkflowTable.vue', 'SoftwareTable.vue', 'DictionaryProof.vue', 'CaptureView.vue']
  for (const page of detailPages) {
    const source = readFileSync(join(sourceRoot, 'components', page), 'utf8')
    expect(source, `${basename(page)} must use the shared detail header.`).toContain('<ManagementDetailHeader')
  }

  const workspacePages = ['WorkflowTable.vue', 'SoftwareTable.vue']
  for (const page of workspacePages) {
    const source = readFileSync(join(sourceRoot, 'components', page), 'utf8')
    expect(source, `${basename(page)} must use the shared workspace surface.`).toContain('<ManagementWorkspaceSurface')
    expect(source, `${basename(page)} must render detail content on the application canvas.`).toContain('variant="canvas"')
    expect(source, `${basename(page)} must use the shared form section.`).toContain('<ManagementFormSection')
    expect(source, `${basename(page)} must use the shared form row.`).toContain('<ManagementFormRow')
  }

  const utilityPages = ['SettingsView.vue', 'HelpView.vue']
  for (const page of utilityPages) {
    const source = readFileSync(join(sourceRoot, 'components', page), 'utf8')
    expect(source, `${basename(page)} must use the shared utility-page shell.`).toContain('<UtilityPageShell')
    expect(source, `${basename(page)} must declare its page icon.`).toMatch(/<UtilityPageShell[\s\S]*?icon="i-tabler-[^"]+"/)
  }

  const utilityPageShell = readFileSync(join(sourceRoot, 'components', 'UtilityPageShell.vue'), 'utf8')
  expect(utilityPageShell).toContain('<ManagementPageHeader')
  expect(utilityPageShell).toContain('<ManagementWorkspaceSurface variant="canvas">')
  expect(utilityPageShell).not.toContain('max-width')
  expect(utilityPageShell).not.toContain('padding-inline')
  expect(utilityPageShell).not.toContain('scrollbar-gutter')

  const workspaceSurface = readFileSync(join(sourceRoot, 'components', 'ManagementWorkspaceSurface.vue'), 'utf8')
  const detailHeader = readFileSync(join(sourceRoot, 'components', 'ManagementDetailHeader.vue'), 'utf8')
  expect(workspaceSurface).toContain("variant?: 'surface' | 'canvas'")
  expect(workspaceSurface).toContain("bg-[var(--app-bg)]")
  expect(detailHeader).toContain('backLabel?: string')
  expect(detailHeader).toContain('v-if="backLabel"')

  const formRow = readFileSync(join(sourceRoot, 'components', 'ManagementFormRow.vue'), 'utf8')
  const formSection = readFileSync(join(sourceRoot, 'components', 'ManagementFormSection.vue'), 'utf8')
  expect(formSection).toContain('@container w-full')
  expect(formSection).not.toContain('max-w-[980px]')
  expect(formSection).toContain('rounded-[10px] border border-[var(--border)]')
  expect(formRow).toContain('grid-cols-[184px_minmax(0,1fr)]')
  expect(formRow).toContain('justify-self-end')
  expect(formRow).toContain('@max-[620px]:grid-cols-1')

  const themeSource = readFileSync(join(sourceRoot, 'main.css'), 'utf8')
  expect(themeSource).toContain('--surface-inset:')
  expect(themeSource).toContain('--field-bg:')
})

test('desktop title bar reads the complete version from package metadata', () => {
  const titleBar = readFileSync(join(sourceRoot, 'components', 'TitleBar.vue'), 'utf8')
  const packageJson = JSON.parse(readFileSync(join(packageRoot, 'package.json'), 'utf8')) as { version: string }

  expect(titleBar).toContain("import { version as appVersion } from '../../package.json'")
  expect(titleBar).toContain('v{{ appVersion }}')
  expect(titleBar).not.toMatch(/>v\d+\.\d+<\/span>/)
  expect(packageJson.version).toMatch(/^\d+\.\d+\.\d+$/)
})

test('management tables share pinned compact context and semantic type roles', () => {
  const compactTables = ['WorkflowTable.vue', 'SoftwareTable.vue', 'DictionaryLibrary.vue', 'CaptureView.vue']
  for (const page of compactTables) {
    const source = readFileSync(join(sourceRoot, 'components', page), 'utf8')
    expect(source, `${basename(page)} must pin its object identity column.`).toContain('managementIdentityColumnMeta')
    expect(source, `${basename(page)} must expose a keyboard-focusable table region.`).toContain('management-table-scroll')
    expect(source, `${basename(page)} must name the table region.`).toMatch(/aria-label(?:ledby)?=/)
  }

  const tableInteraction = readFileSync(join(sourceRoot, 'tableInteraction.ts'), 'utf8')
  expect(tableInteraction).toContain('managementSelectionColumnMeta')
  expect(tableInteraction).toContain('managementActionsColumnMeta')
  expect(tableInteraction).toContain('management-table-identity-cell')
  expect(tableInteraction).toContain('management-table-actions-cell')

  const smallTypeViolations = vueSources().flatMap(path => {
    const source = readFileSync(path, 'utf8')
    return [...source.matchAll(/text-\[(?:8|9|10)px\]/g)].map(match => ({
      file: relative(sourceRoot, path).replaceAll('\\', '/'),
      utility: match[0],
    }))
  })
  expect(smallTypeViolations, 'Use semantic type roles instead of 8–10px functional copy.').toEqual([])

  const themeSource = readFileSync(join(sourceRoot, 'main.css'), 'utf8')
  for (const token of ['caption', 'label', 'metadata', 'body', 'section-title', 'page-title']) {
    expect(themeSource).toContain(`--type-${token}:`)
  }
})

test('modal layers stay above sticky tables and transient menus', () => {
  const viteConfig = readFileSync(join(packageRoot, 'vite.config.ts'), 'utf8')
  expect(viteConfig).toContain("overlay: 'z-[80]")
  expect(viteConfig).toContain("content: 'z-[81]")
})

test('native close requests can destroy the accepted window', () => {
  const capability = JSON.parse(readFileSync(
    join(packageRoot, 'src-tauri', 'capabilities', 'default.json'),
    'utf8',
  )) as { permissions?: string[] }

  expect(capability.permissions).toEqual(expect.arrayContaining([
    'core:window:allow-close',
    'core:window:allow-destroy',
  ]))
})

test('desktop review builds load embedded Tauri assets for every Cargo profile', () => {
  const cargoManifest = readFileSync(join(packageRoot, 'src-tauri', 'Cargo.toml'), 'utf8')
  const reviewScript = readFileSync(join(packageRoot, '..', '..', 'scripts', 'review-app.ps1'), 'utf8')

  expect(cargoManifest).toMatch(/\[features\][\s\S]*custom-protocol\s*=\s*\["tauri\/custom-protocol"\]/)
  expect(reviewScript).toContain("'--features', 'custom-protocol'")
})

test('desktop title bar and native bundles share the selected Glyphshift mark', () => {
  const titleBar = readFileSync(join(sourceRoot, 'components', 'TitleBar.vue'), 'utf8')
  const buildScript = readFileSync(join(packageRoot, 'src-tauri', 'build.rs'), 'utf8')
  const tauriConfig = JSON.parse(readFileSync(join(packageRoot, 'src-tauri', 'tauri.conf.json'), 'utf8')) as {
    bundle?: { icon?: string[] }
  }

  expect(titleBar).toContain("../../src-tauri/icons/icon.svg?url")
  expect(titleBar).toContain('data-testid="glyphshift-mark"')
  expect(titleBar).not.toContain('>G</span>')

  expect(existsSync(join(packageRoot, 'src-tauri', 'icons', 'icon.svg'))).toBe(true)
  expect(tauriConfig.bundle?.icon).toEqual([
    'icons/32x32.png',
    'icons/128x128.png',
    'icons/128x128@2x.png',
    'icons/icon.icns',
    'icons/icon.ico',
  ])
  for (const icon of tauriConfig.bundle?.icon ?? []) {
    expect(existsSync(join(packageRoot, 'src-tauri', icon))).toBe(true)
  }
  expect(buildScript).toContain('cargo:rerun-if-changed=icons/icon.ico')
})

test('adapter documentation opens through a scoped system-browser capability', () => {
  const packageJson = JSON.parse(readFileSync(join(packageRoot, 'package.json'), 'utf8')) as {
    dependencies?: Record<string, string>
  }
  const cargoManifest = readFileSync(join(packageRoot, 'src-tauri/Cargo.toml'), 'utf8')
  const shellSource = readFileSync(join(packageRoot, 'src-tauri/src/lib.rs'), 'utf8')
  const capability = JSON.parse(readFileSync(join(packageRoot, 'src-tauri/capabilities/default.json'), 'utf8')) as {
    permissions?: Array<string | { identifier: string; allow?: Array<{ url?: string }> }>
  }

  expect(packageJson.dependencies?.['@tauri-apps/plugin-opener']).toBeTruthy()
  expect(cargoManifest).toContain('tauri-plugin-opener')
  expect(shellSource).toContain('tauri_plugin_opener::init()')

  const opener = capability.permissions?.find(permission => (
    typeof permission !== 'string' && permission.identifier === 'opener:allow-open-url'
  ))
  expect(opener).toEqual({
    identifier: 'opener:allow-open-url',
    allow: [
      { url: 'https://learn.microsoft.com/*' },
      { url: 'https://doc.qt.io/*' },
      { url: 'https://docs.gtk.org/*' },
      { url: 'https://www.raylib.com/*' },
      { url: 'https://github.com/*' },
      { url: 'https://space.bilibili.com/*' },
    ],
  })
  expect(capability.permissions).not.toContain('opener:default')
  expect(capability.permissions).not.toContain('opener:allow-default-urls')
})

test('probe run keeps the 5000-entry path behind backend paging and revision polling', () => {
  const captureView = readFileSync(join(sourceRoot, 'components', 'CaptureView.vue'), 'utf8')
  const probeRuns = readFileSync(join(sourceRoot, 'useProbeRuns.ts'), 'utf8')

  expect(captureView).toContain('setInterval(() => void poll(), 1000)')
  expect(captureView).toContain(':data="entryPage.rows"')
  expect(probeRuns).toContain("'desktop_probe_run_entries'")
  expect(probeRuns).toContain('pageSize: input.pageSize')
  expect(captureView).toContain('adapterIds: [...adapterFilterIds.value]')
  expect(captureView).toContain('glyphshift.probe.view.${runId}')
  expect(captureView).not.toContain('catalogPage')
  expect(captureView).not.toContain('draftPage')
  expect(captureView).not.toContain('useVirtualList')
})

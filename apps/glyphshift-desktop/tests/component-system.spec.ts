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
  const pages = ['WorkflowTable.vue', 'SoftwareTable.vue', 'DictionaryLibrary.vue', 'CaptureView.vue']

  for (const page of pages) {
    const source = readFileSync(join(sourceRoot, 'components', page), 'utf8')
    expect(source, `${basename(page)} must use the shared page header.`).toContain('<ManagementPageHeader')
    expect(source, `${basename(page)} must use the shared table frame.`).toContain('<ManagementTableFrame')
  }

  const dictionaryEditor = readFileSync(join(sourceRoot, 'components', 'DictionaryProof.vue'), 'utf8')
  expect(dictionaryEditor, 'DictionaryProof.vue must use the shared table frame.').toContain('<ManagementTableFrame')

  const detailPages = ['WorkflowTable.vue', 'SoftwareTable.vue', 'DictionaryProof.vue', 'CaptureView.vue', 'SettingsView.vue']
  for (const page of detailPages) {
    const source = readFileSync(join(sourceRoot, 'components', page), 'utf8')
    expect(source, `${basename(page)} must use the shared detail header.`).toContain('<ManagementDetailHeader')
  }

  const workspacePages = ['WorkflowTable.vue', 'SoftwareTable.vue', 'SettingsView.vue']
  for (const page of workspacePages) {
    const source = readFileSync(join(sourceRoot, 'components', page), 'utf8')
    expect(source, `${basename(page)} must use the shared workspace surface.`).toContain('<ManagementWorkspaceSurface')
    expect(source, `${basename(page)} must render detail content on the application canvas.`).toContain('variant="canvas"')
    expect(source, `${basename(page)} must use the shared form section.`).toContain('<ManagementFormSection')
    expect(source, `${basename(page)} must use the shared form row.`).toContain('<ManagementFormRow')
  }

  const workspaceSurface = readFileSync(join(sourceRoot, 'components', 'ManagementWorkspaceSurface.vue'), 'utf8')
  const detailHeader = readFileSync(join(sourceRoot, 'components', 'ManagementDetailHeader.vue'), 'utf8')
  expect(workspaceSurface).toContain("variant?: 'surface' | 'canvas'")
  expect(workspaceSurface).toContain("bg-[var(--app-bg)]")
  expect(detailHeader).toContain('backLabel?: string')
  expect(detailHeader).toContain('v-if="backLabel"')

  const formRow = readFileSync(join(sourceRoot, 'components', 'ManagementFormRow.vue'), 'utf8')
  const formSection = readFileSync(join(sourceRoot, 'components', 'ManagementFormSection.vue'), 'utf8')
  expect(formSection).toContain('mx-auto w-full max-w-[980px]')
  expect(formSection).toContain('rounded-[10px] border border-[var(--border)]')
  expect(formRow).toContain('grid-cols-[184px_minmax(0,1fr)]')
  expect(formRow).toContain('justify-self-end')
  expect(formRow).toContain('@max-[620px]:grid-cols-1')

  const themeSource = readFileSync(join(sourceRoot, 'main.css'), 'utf8')
  expect(themeSource).toContain('--surface-inset:')
  expect(themeSource).toContain('--field-bg:')
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

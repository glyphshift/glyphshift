import type { AdapterOption } from './model'

const presentationKeys: Record<string, string> = {
  ExtTextOutW: 'extTextOut',
  TextOutW: 'textOut',
  'DrawTextW / DrawTextExW': 'drawText',
  GdipDrawString: 'gdiPlus',
  'DirectWrite TextLayout': 'directWrite',
  'Qt Painter drawText': 'qtPainter',
  'GTK 3 / Pango 绘制': 'gtkPango',
  'raylib DrawTextEx': 'raylib',
  'Unity Mono 标准界面': 'unityMono',
  'Unity IL2CPP 标准界面': 'unityIl2cpp',
}

export function adapterSummary(adapter: AdapterOption, translate: (key: string) => string) {
  const key = presentationKeys[adapter.name]
  return key ? translate(`adapterPresentation.${key}.summary`) : adapter.summary
}

export function adapterDisplayName(name: string, translate: (key: string) => string) {
  const key = presentationKeys[name]
  return key ? translate(`adapterPresentation.${key}.name`) : name
}

export function adapterName(adapter: AdapterOption, translate: (key: string) => string) {
  return adapterDisplayName(adapter.name, translate)
}

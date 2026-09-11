import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'
import ui from '@nuxt/ui/vite'

export default defineConfig({
  plugins: [
    vue(),
    ui({
      icon: { clientBundle: { scan: { globInclude: ['src/**/*.{vue,ts}'] } } },
      ui: {
        colors: {
          primary: 'blue',
          neutral: 'zinc',
          warning: 'amber',
        },
        button: {
          slots: { base: 'type-label justify-center font-semibold' },
          compoundVariants: [
            {
              color: 'primary',
              variant: 'solid',
              class: { base: 'btn-primary-contained' },
            },
          ],
        },
        input: {
          slots: { base: 'type-label bg-[var(--field-bg)]' },
        },
        textarea: {
          slots: { base: 'type-label bg-[var(--field-bg)]' },
        },
        select: {
          slots: {
            base: 'type-label bg-[var(--field-bg)]',
            content: 'relative z-[90] rounded-[6px]',
          },
        },
        formField: {
          slots: {
            label: 'type-label font-medium',
            hint: 'type-metadata',
            description: 'type-metadata',
          },
        },
        checkbox: {
          slots: {
            root: 'items-center',
            label: 'type-label',
          },
        },
        dropdownMenu: {
          slots: {
            content: 'z-[60] min-w-32 rounded-[6px]',
            item: 'type-label min-h-8 items-center rounded-[4px]',
            itemTrailingIcon: 'size-4',
          },
        },
        inputMenu: {
          slots: {
            base: 'type-label bg-[var(--field-bg)]',
            content: 'relative z-[90] rounded-[6px]',
            viewport: 'overscroll-contain',
          },
        },
        selectMenu: {
          slots: {
            base: 'type-label bg-[var(--field-bg)]',
            content: 'relative z-[90] rounded-[6px]',
            viewport: 'overscroll-contain',
          },
        },
        popover: {
          slots: { content: 'z-[60] rounded-[6px]' },
        },
        modal: {
          slots: {
            overlay: 'z-[80]',
            content: 'z-[81] rounded-[8px] bg-[var(--surface)]',
            header: 'border-b border-[var(--border)]',
            footer: 'border-t border-[var(--border)]',
          },
        },
        table: {
          slots: {
            root: 'isolate h-full rounded-none',
            base: 'type-label w-full table-fixed',
            thead: 'bg-[var(--surface-subtle)]',
            tbody: 'divide-y divide-[var(--border)]',
            tr: 'group hover:bg-[var(--surface-subtle)]',
            th: 'type-label h-8 px-3 py-2 font-medium text-[var(--text-muted)]',
            td: 'type-label px-3 py-3 text-[var(--text)]',
            empty: 'h-52 p-0',
          },
        },
        empty: {
          slots: {
            root: 'h-full justify-center rounded-none border-0 bg-transparent p-6',
            title: 'type-section-title',
            description: 'type-metadata',
          },
        },
      },
    }),
    tailwindcss(),
  ],
  clearScreen: false,
  server: {
    host: '127.0.0.1',
    port: 1430,
    strictPort: true,
  },
})

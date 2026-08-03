import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'
import ui from '@nuxt/ui/vite'

export default defineConfig({
  plugins: [
    vue(),
    ui({
      ui: {
        colors: {
          primary: 'blue',
          neutral: 'zinc',
          warning: 'amber',
        },
        button: {
          slots: { base: 'justify-center text-[10px] font-semibold' },
          compoundVariants: [
            {
              color: 'primary',
              variant: 'solid',
              class: { base: 'btn-primary-contained' },
            },
          ],
        },
        input: {
          slots: { base: 'bg-[var(--field-bg)] text-[11px]' },
        },
        textarea: {
          slots: { base: 'bg-[var(--field-bg)] text-[11px]' },
        },
        select: {
          slots: {
            base: 'bg-[var(--field-bg)] text-[11px]',
            content: 'relative z-[90] rounded-[6px]',
          },
        },
        formField: {
          slots: {
            label: 'text-[10px] font-medium',
            hint: 'text-[9px]',
            description: 'text-[9px]',
          },
        },
        checkbox: {
          slots: {
            root: 'items-center',
            label: 'text-[10px]',
          },
        },
        dropdownMenu: {
          slots: {
            content: 'z-[60] min-w-32 rounded-[6px]',
            item: 'min-h-8 items-center rounded-[4px] text-[10px]',
            itemTrailingIcon: 'size-4',
          },
        },
        selectMenu: {
          slots: {
            base: 'bg-[var(--field-bg)] text-[11px]',
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
            root: 'h-full rounded-none',
            base: 'w-full table-fixed text-[11px]',
            thead: 'bg-[var(--surface-subtle)]',
            tbody: 'divide-y divide-[var(--border)]',
            tr: 'hover:bg-[var(--surface-subtle)]',
            th: 'h-8 px-3 py-2 text-[9px] font-medium text-[var(--text-muted)]',
            td: 'px-3 py-3 text-[11px] text-[var(--text)]',
            empty: 'h-52 p-0',
          },
        },
        empty: {
          slots: {
            root: 'h-full justify-center rounded-none border-0 bg-transparent p-6',
            title: 'text-[13px]',
            description: 'text-[10px]',
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

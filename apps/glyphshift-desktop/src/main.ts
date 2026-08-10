import { createApp } from 'vue'
import ui from '@nuxt/ui/vue-plugin'
import { createMemoryHistory, createRouter } from 'vue-router'
import App from './App.vue'
import { initializeAppSettings, reapplyAppSettings } from './appSettings'
import { i18n } from './i18n'
import './main.css'

async function bootstrap() {
  await initializeAppSettings()
  const app = createApp(App)
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [{ path: '/', component: { template: '<span />' } }],
  })
  app.use(router).use(i18n).use(ui)
  app.mount('#app')
  reapplyAppSettings()
}

void bootstrap()

import { createApp } from 'vue'
import ui from '@nuxt/ui/vue-plugin'
import { useDark } from '@vueuse/core'
import { createMemoryHistory, createRouter } from 'vue-router'
import App from './App.vue'
import './main.css'

const app = createApp(App)
const router = createRouter({
  history: createMemoryHistory(),
  routes: [{ path: '/', component: { template: '<span />' } }],
})
app.use(router).use(ui)
useDark().value = true
app.mount('#app')

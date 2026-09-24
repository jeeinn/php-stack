<script setup lang="ts">
import type { ConnectInfo } from '../utils/connectHost'

defineProps<{
  info: ConnectInfo
}>()

const emit = defineEmits<{
  copy: [host: string]
}>()
</script>

<template>
  <div class="mt-3 pt-3 border-t border-slate-200 dark:border-slate-700/50 text-xs text-slate-600 dark:text-slate-400">
    <div class="flex flex-wrap items-center gap-2">
      <span>{{ $t('envConfig.connectHost.label') }}</span>
      <code
        class="px-1.5 py-0.5 rounded bg-slate-200/80 dark:bg-slate-900 font-mono text-slate-800 dark:text-slate-200"
      >{{ info.primary }}</code>
      <button
        type="button"
        class="text-blue-600 dark:text-blue-400 hover:underline"
        @click="emit('copy', info.primary)"
      >
        {{ $t('envConfig.connectHost.copy') }}
      </button>
    </div>
    <p v-if="info.alsoAvailable.length > 0" class="mt-1">
      {{ $t('envConfig.connectHost.alsoAlias', { alias: info.alsoAvailable.join(', ') }) }}
    </p>
    <p class="mt-1 text-slate-500 dark:text-slate-500">
      {{ $t('envConfig.connectHost.portHint', { port: info.containerPort }) }}
    </p>
  </div>
</template>

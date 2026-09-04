<script setup lang="ts">
/**
 * VersionHelpModal.vue
 *
 * 点击版本下拉框旁的 ? 按钮弹出，向用户解释：
 * 1. {name}{version} 命名约定为何不能随便改
 * 2. 如何在自己的 services/version_manifest.json 中添加新版本
 * 3. 添加步骤（manifest 条目 → 模板目录 → 验证）
 * 4. MySQL 5.6 完整 JSON 示例
 * 5. 常见问题排查
 *
 * 设计：内容硬编码，不引入 markdown 渲染依赖。
 * 完整原文见 doc/guides/ADDING_SERVICE_VERSION.md。
 */
import { useI18n } from 'vue-i18n';

const { t: $t } = useI18n();

defineProps<{
  open: boolean;
  // 可选：弹窗打开时聚焦到哪一节（"naming" | "steps" | "sample" | "faq"），默认 "naming"
  initialSection?: 'naming' | 'steps' | 'sample' | 'faq';
}>();

const emit = defineEmits<{
  (e: 'close'): void;
}>();
</script>

<template>
  <div
    v-if="open"
    class="fixed inset-0 bg-black/70 backdrop-blur-sm flex items-center justify-center z-50 p-3 sm:p-4"
    @click.self="emit('close')"
  >
    <div class="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-700 rounded-xl w-full max-w-3xl mx-auto max-h-[90vh] flex flex-col shadow-2xl">
      <!-- Header -->
      <div class="flex justify-between items-center p-4 sm:p-5 border-b border-slate-200 dark:border-slate-700 shrink-0">
        <div>
          <h2 class="text-lg sm:text-xl font-bold text-slate-900 dark:text-slate-200">
            {{ $t('envConfig.versionHelp.title') }}
          </h2>
          <p class="text-xs text-slate-500 dark:text-slate-400 mt-1">
            {{ $t('envConfig.versionHelp.subtitle') }}
          </p>
        </div>
        <button
          @click="emit('close')"
          class="text-slate-500 dark:text-slate-400 hover:text-slate-700 dark:hover:text-slate-200 text-2xl leading-none"
        >
          &times;
        </button>
      </div>

      <!-- Body: scrollable -->
      <div class="flex-1 overflow-y-auto p-4 sm:p-6 space-y-5 text-sm text-slate-700 dark:text-slate-300">
        <!-- §1 命名约定 -->
        <section>
          <h3 class="text-base font-semibold text-blue-600 dark:text-blue-400 mb-2">
            {{ $t('envConfig.versionHelp.sectionNaming') }}
          </h3>
          <p class="mb-3 leading-relaxed">{{ $t('envConfig.versionHelp.namingDesc1') }}</p>
          <div class="overflow-x-auto rounded-lg border border-slate-200 dark:border-slate-700 mb-3">
            <table class="min-w-full text-xs">
              <thead class="bg-slate-50 dark:bg-slate-800">
                <tr>
                  <th class="px-3 py-2 text-left font-medium">{{ $t('envConfig.versionHelp.colService') }}</th>
                  <th class="px-3 py-2 text-left font-medium">{{ $t('envConfig.versionHelp.colVersion') }}</th>
                  <th class="px-3 py-2 text-left font-medium">{{ $t('envConfig.versionHelp.colDir') }}</th>
                  <th class="px-3 py-2 text-left font-medium">{{ $t('envConfig.versionHelp.colId') }}</th>
                </tr>
              </thead>
              <tbody class="divide-y divide-slate-200 dark:divide-slate-700 font-mono">
                <tr><td class="px-3 py-1.5">PHP</td><td class="px-3 py-1.5">8.5</td><td class="px-3 py-1.5">php85</td><td class="px-3 py-1.5">php85</td></tr>
                <tr><td class="px-3 py-1.5">MySQL</td><td class="px-3 py-1.5">8.4</td><td class="px-3 py-1.5">mysql84</td><td class="px-3 py-1.5">mysql84</td></tr>
                <tr><td class="px-3 py-1.5">Redis</td><td class="px-3 py-1.5">7.2</td><td class="px-3 py-1.5">redis72</td><td class="px-3 py-1.5">redis72</td></tr>
                <tr><td class="px-3 py-1.5">Nginx</td><td class="px-3 py-1.5">1.28</td><td class="px-3 py-1.5">nginx128</td><td class="px-3 py-1.5">nginx128</td></tr>
              </tbody>
            </table>
          </div>
          <p class="leading-relaxed text-slate-600 dark:text-slate-400">{{ $t('envConfig.versionHelp.namingDesc2') }}</p>
        </section>

        <!-- §2 三步流程 -->
        <section>
          <h3 class="text-base font-semibold text-blue-600 dark:text-blue-400 mb-2">
            {{ $t('envConfig.versionHelp.sectionSteps') }}
          </h3>

          <div class="space-y-3">
            <div class="rounded-lg border border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-800/40 p-3">
              <p class="font-medium text-slate-900 dark:text-slate-200 mb-1.5">
                {{ $t('envConfig.versionHelp.step1Title') }}
              </p>
              <p class="text-slate-600 dark:text-slate-400 mb-2 text-xs">{{ $t('envConfig.versionHelp.step1Desc') }}</p>
              <pre class="text-[11px] bg-slate-100 dark:bg-black/40 p-2 rounded font-mono overflow-x-auto">{{ $t('envConfig.versionHelp.step1Sample') }}</pre>
            </div>

            <div class="rounded-lg border border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-800/40 p-3">
              <p class="font-medium text-slate-900 dark:text-slate-200 mb-1.5">
                {{ $t('envConfig.versionHelp.step2Title') }}
              </p>
              <ul class="text-xs text-slate-600 dark:text-slate-400 space-y-1.5 list-disc list-inside">
                <li><strong>{{ $t('envConfig.versionHelp.step2A') }}</strong> {{ $t('envConfig.versionHelp.step2ADesc') }}</li>
                <li><strong>{{ $t('envConfig.versionHelp.step2B') }}</strong> {{ $t('envConfig.versionHelp.step2BDesc') }}</li>
                <li><strong>{{ $t('envConfig.versionHelp.step2C') }}</strong> {{ $t('envConfig.versionHelp.step2CDesc') }}</li>
              </ul>
            </div>

            <div class="rounded-lg border border-amber-300 dark:border-amber-700 bg-amber-50 dark:bg-amber-950/30 p-3">
              <p class="font-medium text-amber-900 dark:text-amber-200 mb-1 text-xs">
                ⚠️ {{ $t('envConfig.versionHelp.step3Title') }}
              </p>
              <p class="text-xs text-amber-800 dark:text-amber-300 leading-relaxed">
                {{ $t('envConfig.versionHelp.step3Desc') }}
              </p>
            </div>
          </div>
        </section>

        <!-- §3 MySQL 5.6 完整示例 -->
        <section>
          <h3 class="text-base font-semibold text-blue-600 dark:text-blue-400 mb-2">
            {{ $t('envConfig.versionHelp.sectionSample') }}
          </h3>
          <p class="text-slate-600 dark:text-slate-400 mb-2 text-xs">{{ $t('envConfig.versionHelp.sampleDesc') }}</p>
          <pre class="text-[11px] bg-slate-100 dark:bg-black/40 p-3 rounded font-mono overflow-x-auto leading-relaxed">{{ $t('envConfig.versionHelp.mysql56Sample') }}</pre>
          <p class="text-xs text-slate-500 dark:text-slate-500 mt-2">
            💡 {{ $t('envConfig.versionHelp.sampleTip') }}
          </p>
        </section>

        <!-- §4 常见问题 -->
        <section>
          <h3 class="text-base font-semibold text-blue-600 dark:text-blue-400 mb-2">
            {{ $t('envConfig.versionHelp.sectionFaq') }}
          </h3>
          <div class="space-y-2 text-xs">
            <details class="rounded border border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-800/40 px-3 py-2">
              <summary class="cursor-pointer font-medium text-slate-700 dark:text-slate-300">{{ $t('envConfig.versionHelp.faq1Q') }}</summary>
              <p class="mt-2 text-slate-600 dark:text-slate-400 leading-relaxed">{{ $t('envConfig.versionHelp.faq1A') }}</p>
            </details>
            <details class="rounded border border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-800/40 px-3 py-2">
              <summary class="cursor-pointer font-medium text-slate-700 dark:text-slate-300">{{ $t('envConfig.versionHelp.faq2Q') }}</summary>
              <p class="mt-2 text-slate-600 dark:text-slate-400 leading-relaxed">{{ $t('envConfig.versionHelp.faq2A') }}</p>
            </details>
            <details class="rounded border border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-800/40 px-3 py-2">
              <summary class="cursor-pointer font-medium text-slate-700 dark:text-slate-300">{{ $t('envConfig.versionHelp.faq3Q') }}</summary>
              <p class="mt-2 text-slate-600 dark:text-slate-400 leading-relaxed">{{ $t('envConfig.versionHelp.faq3A') }}</p>
            </details>
          </div>
        </section>

        <!-- footer 文档路径提示 -->
        <div class="pt-3 border-t border-slate-200 dark:border-slate-700 text-xs text-slate-500 dark:text-slate-500">
          {{ $t('envConfig.versionHelp.docFooter') }}
        </div>
      </div>

      <!-- Footer -->
      <div class="p-4 sm:p-5 border-t border-slate-200 dark:border-slate-700 flex justify-end shrink-0">
        <button
          @click="emit('close')"
          class="px-5 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded-lg font-medium transition"
        >
          {{ $t('common.close') }}
        </button>
      </div>
    </div>
  </div>
</template>

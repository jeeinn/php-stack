import { computed, ref } from 'vue';

/** Set by a silent startup check; About page and the banner share this. */
export const pendingUpdateVersion = ref('');

/** 用户点「查看」或关闭后隐藏顶栏；换新版本号时重新显示 */
const bannerDismissed = ref(false);

export const showUpdateBanner = computed(
  () => !!pendingUpdateVersion.value && !bannerDismissed.value,
);

export function setPendingUpdateVersion(version: string) {
  if (version !== pendingUpdateVersion.value) {
    bannerDismissed.value = false;
  }
  pendingUpdateVersion.value = version;
  if (!version) {
    bannerDismissed.value = false;
  }
}

export function dismissUpdateBanner() {
  bannerDismissed.value = true;
}

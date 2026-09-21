import { ref } from 'vue';

/** Set by a silent startup check; About page and the banner share this. */
export const pendingUpdateVersion = ref('');

export function setPendingUpdateVersion(version: string) {
  pendingUpdateVersion.value = version;
}

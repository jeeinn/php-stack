/**
 * 容器轮询间隔与退避策略。
 *
 * Docker Desktop 未启动时，固定 5 秒一次的全量轮询（ping + 拉列表 + 写日志）
 * 会持续刷屏且毫无产出。连续失败后拉长间隔，恢复可用后立即回到 5 秒。
 */

/** Docker 可用时的正常轮询间隔 */
export const POLL_INTERVAL_MS = 5000;

/** 连续失败达到阈值后逐级退避的间隔 */
const POLL_BACKOFF_MS = [15000, 30000];

/** 触发各档退避所需的连续失败次数（与 POLL_BACKOFF_MS 一一对应） */
const BACKOFF_THRESHOLDS = [3, 6];

/**
 * 根据连续失败次数得出下一次轮询的等待时间。
 *
 * 0~2 次失败：5s（短暂抖动不惩罚）
 * 3~5 次失败：15s
 * 6 次及以上：30s
 */
export function nextPollDelay(consecutiveFailures: number): number {
  // 从最严档位往下找第一个满足的阈值
  for (let i = BACKOFF_THRESHOLDS.length - 1; i >= 0; i--) {
    if (consecutiveFailures >= BACKOFF_THRESHOLDS[i]) {
      return POLL_BACKOFF_MS[i];
    }
  }
  return POLL_INTERVAL_MS;
}

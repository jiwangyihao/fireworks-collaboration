import { getIpPoolSnapshot, type IpPoolSnapshot } from "../api/ip-pool";

export type PreheatWarmupState =
  | { state: "disabled" }
  | { state: "inactive"; totalTargets: number; completedTargets: number }
  | { state: "ready"; totalTargets: number; completedTargets: number }
  | { state: "pending"; totalTargets: number; completedTargets: number };

export function extractProgress(snapshot: IpPoolSnapshot): {
  totalTargets: number;
  completedTargets: number;
} {
  const totalTargets = snapshot.preheatTargets ?? 0;
  const completedTargets = Math.min(snapshot.preheatedTargets ?? 0, totalTargets);
  return { totalTargets, completedTargets };
}

function delay(ms: number) {
  return new Promise<void>((resolve) => setTimeout(resolve, ms));
}

export async function waitForIpPoolWarmup(
  initialTotalTargets = 0,
  attempts = 12,
  intervalMs = 1_000,
): Promise<PreheatWarmupState> {
  let lastPreheatEnabled = false;
  let lastTotalTargets = initialTotalTargets;
  let lastCompletedTargets = 0;
  let sawSnapshot = false;

  for (let attempt = 0; attempt < attempts; attempt += 1) {
    try {
      const snapshot = await getIpPoolSnapshot();
      sawSnapshot = true;
      if (!snapshot.enabled) {
        return { state: "disabled" };
      }

      const { totalTargets, completedTargets } = extractProgress(snapshot);
      lastPreheatEnabled = snapshot.preheatEnabled;
      lastTotalTargets = totalTargets;
      lastCompletedTargets = completedTargets;

      if (!snapshot.preheatEnabled) {
        return { state: "inactive", totalTargets, completedTargets };
      }

      if (totalTargets === 0) {
        return { state: "ready", totalTargets, completedTargets };
      }

      if (completedTargets >= totalTargets) {
        return { state: "ready", totalTargets, completedTargets };
      }
    } catch (error) {
      console.warn("获取 IP 池快照失败，稍后重试", error);
    }
    await delay(intervalMs);
  }

  if (!sawSnapshot || !lastPreheatEnabled) {
    return {
      state: "inactive",
      totalTargets: lastTotalTargets,
      completedTargets: lastCompletedTargets,
    };
  }

  return {
    state: "pending",
    totalTargets: lastTotalTargets,
    completedTargets: lastCompletedTargets,
  };
}

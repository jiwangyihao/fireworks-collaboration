import { defineStore } from "pinia";
import type { AppConfig } from "../api/config";
import { getConfig, setConfig } from "../api/config";

export const useConfigStore = defineStore("config", {
  state: () => ({
    cfg: null as AppConfig | null,
    loading: false,
    error: null as string | null,
  }),
  actions: {
    async refresh() {
      this.loading = true;
      this.error = null;
      try {
        this.cfg = await getConfig();
      } catch (e: any) {
        this.error = String(e);
      } finally {
        this.loading = false;
      }
    },
    async save(next: AppConfig) {
      this.loading = true;
      this.error = null;
      try {
        await setConfig(next);
        this.cfg = next;
      } catch (e: any) {
        this.error = String(e);
        throw e;
      } finally {
        this.loading = false;
      }
    },
  },
});

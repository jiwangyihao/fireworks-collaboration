<template>
  <div class="p-4 pt-16 space-y-3">
    <div class="flex gap-2">
      <select v-model="method" class="select select-bordered select-sm w-28">
        <option>GET</option>
        <option>POST</option>
        <option>PUT</option>
        <option>DELETE</option>
      </select>
      <input v-model="url" placeholder="https://github.com/" class="input input-bordered input-sm flex-1" />
      <button class="btn btn-primary btn-sm" @click="send">Send</button>
    </div>
    <div class="grid grid-cols-2 gap-3">
      <div>
        <h3 class="font-semibold">Headers</h3>
        <textarea v-model="headersText" class="textarea textarea-bordered w-full h-40" placeholder='{"User-Agent":"P0Test"}' />
        <h3 class="font-semibold mt-2">Body (text)</h3>
        <textarea v-model="bodyText" class="textarea textarea-bordered w-full h-36" placeholder="optional body" />
        <div class="flex gap-2 mt-2">
          <label class="label cursor-pointer gap-2"><span>Force Real SNI</span><input type="checkbox" v-model="forceRealSni" class="checkbox checkbox-sm" /></label>
          <label class="label cursor-pointer gap-2"><span>Follow Redirects</span><input type="checkbox" v-model="followRedirects" class="checkbox checkbox-sm" /></label>
        </div>
        <div class="mt-2 p-2 border rounded">
          <div class="flex items-center gap-2">
            <label class="label cursor-pointer gap-2">
              <span>跳过证书验证（不安全，原型期）</span>
              <input type="checkbox" v-model="insecureSkipVerify" class="checkbox checkbox-sm" @change="applyTlsToggle" />
            </label>
            <span class="text-xs opacity-70">启用后 TLS 将不校验证书链与域名，仅用于验证伪 SNI 的可达性。</span>
          </div>
          <div class="grid grid-cols-2 gap-2 mt-2">
            <label class="label cursor-pointer gap-2"><span>启用 Fake SNI</span><input type="checkbox" v-model="fakeSniEnabled" class="checkbox checkbox-sm" /></label>
            <input v-model="fakeSniHost" class="input input-bordered input-sm" placeholder="伪 SNI 域名，如 baidu.com" />
            <button class="btn btn-sm" @click="applyHttpStrategy">保存 HTTP 策略</button>
          </div>
        </div>
        <div class="mt-3">
          <h3 class="font-semibold">最近请求</h3>
          <div class="text-xs opacity-60 mb-1">保留最近 10 条，点击可回填</div>
          <ul class="menu bg-base-200 rounded-box text-sm">
            <li v-for="h in history" :key="h.key">
              <a @click="applyHistory(h)">{{ h.method }} {{ h.url }}</a>
            </li>
            <li v-if="history.length===0" class="opacity-60 p-2">暂无历史</li>
          </ul>
        </div>
      </div>
      <div>
        <h3 class="font-semibold">Response</h3>
        <div v-if="resp">
          <div class="text-sm">Status: <b>{{ resp.status }}</b> | usedFakeSni: {{ resp.usedFakeSni }}</div>
          <div class="text-sm">Timing: connect {{ resp.timing.connectMs }}ms, tls {{ resp.timing.tlsMs }}ms, firstByte {{ resp.timing.firstByteMs }}ms, total {{ resp.timing.totalMs }}ms</div>
          <div class="text-sm">Size: {{ resp.bodySize }} bytes</div>
          <div class="text-sm" v-if="resp.redirects.length>0">Redirects:
            <ul class="list-disc ml-6">
              <li v-for="r in resp.redirects" :key="r.count">#{{ r.count }} {{ r.status }} -> {{ r.location }}</li>
            </ul>
          </div>
          <h4 class="font-semibold mt-2">Headers</h4>
          <pre class="text-xs whitespace-pre-wrap">{{ resp.headers }}</pre>
          <h4 class="font-semibold mt-2">Body (utf-8)</h4>
          <pre class="text-xs whitespace-pre-wrap">{{ decodedText }}</pre>
        </div>
        <div v-else class="opacity-60">No response yet</div>
        <div v-if="err" class="text-red-600 mt-2">Error: {{ err }}</div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { httpFakeRequest, type HttpRequestInput, type HttpResponseOutput } from "../api/http";
import { getConfig, setConfig, type AppConfig } from "../api/config";
import { useLogsStore } from "../stores/logs";

const method = ref("GET");
const url = ref("https://github.com/");
const headersText = ref('{"User-Agent":"P0Test"}');
const bodyText = ref("");
const forceRealSni = ref(false);
const followRedirects = ref(true);
const insecureSkipVerify = ref(false);
const fakeSniEnabled = ref(true);
const fakeSniHost = ref("baidu.com");

const resp = ref<HttpResponseOutput | null>(null);
const err = ref<string | null>(null);
const logs = useLogsStore();
const history = ref<{ key: string; url: string; method: string; headers: string; bodyText: string; forceRealSni: boolean; followRedirects: boolean }[]>([]);

const decodedText = computed(() => {
  if (!resp.value) return "";
  try {
    const bytes = atob(resp.value.bodyBase64);
    return bytes;
  } catch {
    return "<binary>";
  }
});

async function send() {
  err.value = null; resp.value = null;
  let headers: Record<string, string> = {};
  try { headers = JSON.parse(headersText.value || "{}"); } catch(e) { err.value = "Headers JSON 解析失败"; return; }
  const req: HttpRequestInput = {
    url: url.value,
    method: method.value,
    headers,
    bodyBase64: bodyText.value ? btoa(bodyText.value) : null,
    timeoutMs: 30000,
    forceRealSni: forceRealSni.value,
    followRedirects: followRedirects.value,
    maxRedirects: 5,
  };
  try {
    const out = await httpFakeRequest(req);
    resp.value = out;
    // 记录历史
    const item = { key: Date.now()+":"+Math.random(), url: url.value, method: method.value, headers: headersText.value, bodyText: bodyText.value, forceRealSni: forceRealSni.value, followRedirects: followRedirects.value };
    history.value.unshift(item);
    if (history.value.length > 10) history.value.pop();
  } catch (e:any) {
    err.value = String(e);
    logs.push("error", `HTTP 请求失败: ${err.value}`);
  }
}

// 加载/应用 TLS 跳过验证开关
const cfg = ref<AppConfig | null>(null);
onMounted(async () => {
  try {
    cfg.value = await getConfig();
    insecureSkipVerify.value = !!cfg.value.tls.insecureSkipVerify;
    fakeSniEnabled.value = !!cfg.value.http.fakeSniEnabled;
    fakeSniHost.value = cfg.value.http.fakeSniHost || "baidu.com";
  } catch (e) {
    // 忽略读取失败
  }
});

async function applyTlsToggle() {
  try {
    if (!cfg.value) {
      cfg.value = await getConfig();
    }
    cfg.value!.tls.insecureSkipVerify = insecureSkipVerify.value;
    await setConfig(cfg.value!);
  } catch (e) {
    // 简单提示：可根据需要接入全局 toast
    console.error("更新配置失败", e);
    logs.push("error", `更新 TLS 配置失败: ${String(e)}`);
  }
}

async function applyHttpStrategy() {
  try {
    if (!cfg.value) cfg.value = await getConfig();
    cfg.value!.http.fakeSniEnabled = !!fakeSniEnabled.value;
    cfg.value!.http.fakeSniHost = (fakeSniHost.value || '').trim() || 'baidu.com';
    await setConfig(cfg.value!);
  } catch (e) {
    console.error("更新 HTTP 策略失败", e);
    logs.push("error", `更新 HTTP 策略失败: ${String(e)}`);
  }
}

function applyHistory(h: { url: string; method: string; headers: string; bodyText: string; forceRealSni: boolean; followRedirects: boolean }){
  url.value = h.url;
  method.value = h.method;
  headersText.value = h.headers;
  bodyText.value = h.bodyText;
  forceRealSni.value = h.forceRealSni;
  followRedirects.value = h.followRedirects;
}
</script>

<style scoped>
</style>

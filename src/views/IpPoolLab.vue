<template>
  <div class="space-y-6 p-6 pt-16">
    <div class="flex flex-col gap-1 md:flex-row md:items-end md:justify-between">
      <div>
        <h1 class="text-2xl font-semibold">IP 池实验室</h1>
        <p class="text-sm text-base-content/60">配置运行参数、管理预热列表，并验证候选选择行为。</p>
      </div>
      <div class="flex gap-2">
        <button
          class="btn btn-sm btn-outline"
          type="button"
          :disabled="loading"
          data-testid="reload-snapshot"
          @click="loadSnapshot"
        >
          <span v-if="loading" class="loading loading-spinner loading-xs"></span>
          <span v-else>刷新数据</span>
        </button>
        <button
          class="btn btn-sm btn-primary"
          type="button"
          :disabled="saveDisabled"
          data-testid="save-config"
          @click="handleSave"
        >
          <span v-if="saving" class="loading loading-spinner loading-xs"></span>
          <span v-else>保存配置</span>
        </button>
      </div>
    </div>

    <div v-if="loadError" class="alert alert-error shadow-sm">
      <span>加载 IP 池状态失败：{{ loadError }}</span>
    </div>
    <div v-if="saveError" class="alert alert-error shadow-sm">
      <span>保存失败：{{ saveError }}</span>
    </div>
    <div v-if="refreshMessage" class="alert alert-info shadow-sm">
      <span>{{ refreshMessage }}</span>
    </div>

    <div v-if="snapshot" class="space-y-6">
      <div class="grid gap-4 xl:grid-cols-2">
        <section class="card bg-base-100 shadow-sm">
          <div class="card-body space-y-4">
            <header class="flex items-center justify-between">
              <h2 class="card-title text-lg">运行期配置</h2>
              <span class="badge badge-outline" :class="runtimeForm.enabled ? 'badge-success' : 'badge-ghost'">
                {{ runtimeForm.enabled ? '已启用' : '未启用' }}
              </span>
            </header>
            <div class="grid gap-3 md:grid-cols-2">
              <label class="form-control">
                <span class="label-text">并发探测上限</span>
                <input
                  v-model.number="runtimeForm.maxParallelProbes"
                  type="number"
                  min="1"
                  class="input input-sm input-bordered"
                />
              </label>
              <label class="form-control">
                <span class="label-text">握手超时 (ms)</span>
                <input
                  v-model.number="runtimeForm.probeTimeoutMs"
                  type="number"
                  min="100"
                  class="input input-sm input-bordered"
                />
              </label>
              <label class="form-control">
                <span class="label-text">单飞等待超时 (ms)</span>
                <input
                  v-model.number="runtimeForm.singleflightTimeoutMs"
                  type="number"
                  min="100"
                  class="input input-sm input-bordered"
                />
              </label>
              <label class="form-control">
                <span class="label-text">缓存尺寸上限 (0=无限)</span>
                <input
                  v-model.number="runtimeForm.maxCacheEntries"
                  type="number"
                  min="0"
                  class="input input-sm input-bordered"
                />
              </label>
              <label class="form-control">
                <span class="label-text">缓存清理周期 (秒)</span>
                <input
                  v-model.number="runtimeForm.cachePruneIntervalSecs"
                  type="number"
                  min="5"
                  class="input input-sm input-bordered"
                />
              </label>
              <label class="form-control">
                <span class="label-text">熔断冷却时间 (秒)</span>
                <input
                  v-model.number="runtimeForm.cooldownSeconds"
                  type="number"
                  min="0"
                  class="input input-sm input-bordered"
                />
              </label>
            </div>
            <div class="grid gap-3 md:grid-cols-3">
              <label class="form-control">
                <span class="label-text">连续失败阈值</span>
                <input
                  v-model.number="runtimeForm.failureThreshold"
                  type="number"
                  min="1"
                  class="input input-sm input-bordered"
                />
              </label>
              <label class="form-control">
                <span class="label-text">失败率阈值 (0-1)</span>
                <input
                  v-model.number="runtimeForm.failureRateThreshold"
                  type="number"
                  min="0"
                  max="1"
                  step="0.05"
                  class="input input-sm input-bordered"
                />
              </label>
              <label class="form-control">
                <span class="label-text">统计窗口 (秒)</span>
                <input
                  v-model.number="runtimeForm.failureWindowSeconds"
                  type="number"
                  min="1"
                  class="input input-sm input-bordered"
                />
              </label>
              <label class="form-control">
                <span class="label-text">最小样本数</span>
                <input
                  v-model.number="runtimeForm.minSamplesInWindow"
                  type="number"
                  min="1"
                  class="input input-sm input-bordered"
                />
              </label>
              <label class="form-control md:col-span-2">
                <span class="label-text">历史缓存路径 (可选)</span>
                <input
                  v-model="historyPathText"
                  type="text"
                  class="input input-sm input-bordered"
                  placeholder="例如 config/ip-history.json"
                />
              </label>
            </div>
            <div class="grid gap-2 sm:grid-cols-3">
              <label class="label cursor-pointer justify-start gap-2" data-testid="runtime-enabled">
                <input v-model="runtimeForm.enabled" type="checkbox" class="checkbox checkbox-sm" />
                <span class="label-text">启用 IP 池</span>
              </label>
              <label class="label cursor-pointer justify-start gap-2">
                <input v-model="runtimeForm.circuitBreakerEnabled" type="checkbox" class="checkbox checkbox-sm" />
                <span class="label-text">启用熔断</span>
              </label>
              <label class="label cursor-pointer justify-start gap-2">
                <input v-model="runtimeForm.sources.builtin" type="checkbox" class="checkbox checkbox-sm" />
                <span class="label-text">内置候选</span>
              </label>
              <label class="label cursor-pointer justify-start gap-2">
                <input v-model="runtimeForm.sources.dns" type="checkbox" class="checkbox checkbox-sm" />
                <span class="label-text">DNS 来源</span>
              </label>
              <label class="label cursor-pointer justify-start gap-2">
                <input v-model="runtimeForm.sources.history" type="checkbox" class="checkbox checkbox-sm" />
                <span class="label-text">历史缓存</span>
              </label>
              <label class="label cursor-pointer justify-start gap-2">
                <input v-model="runtimeForm.sources.userStatic" type="checkbox" class="checkbox checkbox-sm" />
                <span class="label-text">用户静态</span>
              </label>
              <label class="label cursor-pointer justify-start gap-2">
                <input v-model="runtimeForm.sources.fallback" type="checkbox" class="checkbox checkbox-sm" />
                <span class="label-text">兜底候选</span>
              </label>
            </div>
            <div class="space-y-3">
              <div class="flex items-center justify-between">
                <h3 class="font-semibold">DNS 解析</h3>
                <button class="btn btn-xs" type="button" @click="addDnsResolver">添加解析器</button>
              </div>
              <label class="label cursor-pointer justify-start gap-2">
                <input v-model="runtimeForm.dns.useSystem" type="checkbox" class="checkbox checkbox-sm" />
                <span class="label-text">使用系统 DNS</span>
              </label>
              <div v-if="dnsResolversForm.length === 0" class="text-sm text-base-content/60">
                未配置自定义解析器。
              </div>
              <div
                v-for="(resolver, index) in dnsResolversForm"
                :key="`dns-${index}`"
                class="rounded-lg border border-base-200 p-3 space-y-2"
              >
                <div class="grid gap-2 md:grid-cols-2">
                  <label class="form-control">
                    <span class="label-text">名称</span>
                    <input
                      v-model="resolver.label"
                      type="text"
                      class="input input-sm input-bordered"
                      placeholder="例如 Cloudflare DoH"
                    />
                  </label>
                  <label class="form-control">
                    <span class="label-text">协议</span>
                    <select v-model="resolver.protocol" class="select select-sm select-bordered">
                      <option value="doh">DoH</option>
                      <option value="dot">DoT</option>
                      <option value="udp">UDP</option>
                    </select>
                  </label>
                </div>
                <div class="grid gap-2 md:grid-cols-2">
                  <label class="form-control">
                    <span class="label-text">Endpoint</span>
                    <input
                      v-model="resolver.endpoint"
                      type="text"
                      class="input input-sm input-bordered"
                      placeholder="cloudflare-dns.com"
                    />
                  </label>
                  <label class="form-control">
                    <span class="label-text">端口 (可选)</span>
                    <input
                      v-model="resolver.portText"
                      type="text"
                      class="input input-sm input-bordered"
                      placeholder="默认 443 或 53"
                    />
                  </label>
                </div>
                <label class="form-control">
                  <span class="label-text">Bootstrap IP (可选)</span>
                  <textarea
                    v-model="resolver.bootstrapText"
                    class="textarea textarea-bordered h-20 text-sm"
                    placeholder="1.1.1.1, 1.0.0.1"
                  ></textarea>
                </label>
                <div class="grid gap-2 md:grid-cols-2">
                  <label class="form-control">
                    <span class="label-text">TLS SNI (可选)</span>
                    <input
                      v-model="resolver.sni"
                      type="text"
                      class="input input-sm input-bordered"
                      placeholder="例如 baidu.com"
                    />
                  </label>
                  <label class="form-control">
                    <span class="label-text">缓存上限 (可选)</span>
                    <input
                      v-model="resolver.cacheSizeText"
                      type="number"
                      min="0"
                      class="input input-sm input-bordered"
                      placeholder="默认 0 (禁用缓存)"
                    />
                  </label>
                </div>
                <label class="form-control">
                  <span class="label-text">备注 (可选)</span>
                  <input
                    v-model="resolver.desc"
                    type="text"
                    class="input input-sm input-bordered"
                    placeholder="说明用途或线路"
                  />
                </label>
                <div class="flex justify-end">
                  <button class="btn btn-ghost btn-xs" type="button" @click="removeDnsResolver(index)">删除</button>
                </div>
              </div>
              <div class="space-y-2">
                <div class="flex items-center justify-between">
                  <h4 class="font-semibold">默认 DoH/DoT 列表</h4>
                  <span class="text-xs text-base-content/60">勾选后参与解析。</span>
                </div>
                <div v-if="hasDnsPresets" class="overflow-x-auto rounded-lg border border-base-200">
                  <table class="table table-xs">
                    <thead>
                      <tr class="text-xs uppercase">
                        <th class="w-16">启用</th>
                        <th>名称</th>
                        <th>协议</th>
                        <th>服务器</th>
                        <th>SNI</th>
                        <th>缓存</th>
                        <th>备注</th>
                      </tr>
                    </thead>
                    <tbody>
                      <tr
                        v-for="entry in dnsPresetEntries"
                        :key="entry.key"
                        :class="entry.disabled ? 'opacity-60' : ''"
                      >
                        <td>
                          <input
                            type="checkbox"
                            class="checkbox checkbox-xs"
                            :checked="entry.enabled"
                            :disabled="entry.disabled"
                            @change="togglePreset(entry.key, ($event.target as HTMLInputElement).checked)"
                          />
                        </td>
                        <td class="font-medium">{{ entry.key }}</td>
                        <td>{{ entry.protocol }}</td>
                        <td class="max-w-[200px] truncate" :title="entry.preset.server">{{ entry.preset.server }}</td>
                        <td>{{ entry.preset.sni || '-' }}</td>
                        <td>{{ entry.preset.cacheSize ?? '-' }}</td>
                        <td class="space-x-1">
                          <span v-if="entry.preset.forSNI" class="badge badge-outline badge-xs">SNI</span>
                          <span>{{ entry.preset.desc || '-' }}</span>
                        </td>
                      </tr>
                    </tbody>
                  </table>
                </div>
                <div v-else class="text-sm text-base-content/60">暂无内置 DoH/DoT 列表。</div>
              </div>
            </div>
          </div>
        </section>

        <section class="card bg-base-100 shadow-sm">
          <div class="card-body space-y-4">
            <h2 class="card-title text-lg">预热与静态配置</h2>
            <label class="form-control max-w-xs">
              <span class="label-text">评分 TTL (秒)</span>
              <input
                v-model.number="scoreTtlSeconds"
                type="number"
                min="30"
                class="input input-sm input-bordered"
              />
            </label>
            <div class="space-y-3">
              <div class="flex items-center justify-between">
                <h3 class="font-semibold">预热域名</h3>
                <button class="btn btn-xs" type="button" @click="addPreheatDomain">添加</button>
              </div>
              <div v-if="preheatDomainsForm.length === 0" class="text-sm text-base-content/60">暂无预热域名。</div>
              <div
                v-for="(item, index) in preheatDomainsForm"
                :key="index"
                class="rounded-lg border border-base-200 p-3 space-y-2"
              >
                <div class="flex gap-2">
                  <label class="form-control flex-1">
                    <span class="label-text">域名</span>
                    <input
                      v-model="item.host"
                      type="text"
                      class="input input-sm input-bordered"
                      :data-testid="`preheat-host-${index}`"
                      placeholder="github.com"
                    />
                  </label>
                  <button class="btn btn-ghost btn-xs mt-6" type="button" @click="removePreheatDomain(index)">删除</button>
                </div>
                <label class="form-control">
                  <span class="label-text">端口列表 (逗号或空格分隔)</span>
                  <input
                    v-model="item.portsText"
                    type="text"
                    class="input input-sm input-bordered"
                    :data-testid="`preheat-ports-${index}`"
                    placeholder="443, 8443"
                  />
                </label>
              </div>
            </div>
            <label class="form-control">
              <span class="label-text">禁用内置预热域名</span>
              <textarea
                v-model="disabledBuiltinText"
                class="textarea textarea-bordered h-28 text-sm"
                placeholder="例如 gist.github.com"
              ></textarea>
              <span class="label-text-alt text-xs text-base-content/60">每行一个域名，不区分大小写。</span>
            </label>
            <div class="space-y-3">
              <div class="flex items-center justify-between">
                <h3 class="font-semibold">静态 IP</h3>
                <button class="btn btn-xs" type="button" @click="addUserStatic">添加</button>
              </div>
              <div v-if="userStaticForm.length === 0" class="text-sm text-base-content/60">暂无静态 IP。</div>
              <div
                v-for="(item, index) in userStaticForm"
                :key="`static-${index}`"
                class="rounded-lg border border-base-200 p-3 space-y-2"
              >
                <div class="grid gap-2 md:grid-cols-2">
                  <label class="form-control">
                    <span class="label-text">域名</span>
                    <input v-model="item.host" type="text" class="input input-sm input-bordered" />
                  </label>
                  <label class="form-control">
                    <span class="label-text">IP 地址</span>
                    <input v-model="item.ip" type="text" class="input input-sm input-bordered" placeholder="140.82.114.3" />
                  </label>
                </div>
                <div class="flex gap-2">
                  <label class="form-control flex-1">
                    <span class="label-text">端口列表</span>
                    <input v-model="item.portsText" type="text" class="input input-sm input-bordered" placeholder="443" />
                  </label>
                  <button class="btn btn-ghost btn-xs mt-6" type="button" @click="removeUserStatic(index)">删除</button>
                </div>
              </div>
            </div>
            <div class="grid gap-3 md:grid-cols-2">
              <label class="form-control">
                <span class="label-text">白名单 (CIDR / IP)</span>
                <textarea v-model="whitelistText" class="textarea textarea-bordered h-28 text-sm" placeholder="例如 140.82.0.0/16"></textarea>
              </label>
              <label class="form-control">
                <span class="label-text">黑名单 (CIDR / IP)</span>
                <textarea v-model="blacklistText" class="textarea textarea-bordered h-28 text-sm" placeholder="例如 10.0.0.0/8"></textarea>
              </label>
            </div>
          </div>
        </section>
      </div>

      <div class="grid gap-4 lg:grid-cols-2">
        <section class="card bg-base-100 shadow-sm">
          <div class="card-body space-y-3">
            <h2 class="card-title text-lg">运行状态</h2>
            <div class="grid gap-2 text-sm">
              <div class="flex items-center justify-between">
                <span class="text-base-content/60">预热线程</span>
                <span class="font-medium" :class="preheaterActive ? 'text-success' : 'text-base-content/60'">
                  {{ preheaterActive ? '运行中' : '未启动' }}
                </span>
              </div>
              <div class="flex items-center justify-between">
                <span class="text-base-content/60">自动禁用状态</span>
                <span class="font-medium" :class="autoDisabledInfo?.active ? 'text-warning' : 'text-base-content'">
                  {{ autoDisabledInfo?.label ?? '未禁用' }}
                </span>
              </div>
              <div class="flex items-center justify-between">
                <span class="text-base-content/60">缓存条目</span>
                <span class="font-medium">{{ cacheEntries.length }}</span>
              </div>
              <div class="flex items-center justify-between">
                <span class="text-base-content/60">更新时间</span>
                <span class="font-medium">{{ lastUpdated }}</span>
              </div>
            </div>
            <div class="flex flex-wrap gap-2">
              <button
                class="btn btn-sm"
                type="button"
                :disabled="loading"
                data-testid="request-refresh"
                @click="handleRefresh"
              >
                触发预热
              </button>
              <button
                class="btn btn-sm btn-outline"
                type="button"
                :disabled="loading"
                @click="handleClearAutoDisable"
              >
                清除禁用
              </button>
            </div>
          </div>
        </section>

        <section class="card bg-base-100 shadow-sm">
          <div class="card-body space-y-3">
            <h2 class="card-title text-lg">候选选择测试</h2>
            <div class="grid gap-2 md:grid-cols-2">
              <label class="form-control">
                <span class="label-text">域名</span>
                <input v-model="testHost" type="text" class="input input-sm input-bordered" />
              </label>
              <label class="form-control">
                <span class="label-text">端口</span>
                <input v-model.number="testPort" type="number" min="1" max="65535" class="input input-sm input-bordered" />
              </label>
            </div>
            <div class="flex gap-2">
              <button class="btn btn-sm btn-primary" type="button" :disabled="testLoading" @click="handlePick">
                <span v-if="testLoading" class="loading loading-spinner loading-xs"></span>
                <span v-else>执行选择</span>
              </button>
              <button class="btn btn-sm btn-ghost" type="button" @click="selectionResult = null">清空结果</button>
            </div>
            <div v-if="testError" class="text-sm text-error">{{ testError }}</div>
            <div v-if="selectionResult" class="rounded-lg border border-base-200 p-3 text-sm space-y-1">
              <div><span class="font-semibold">策略：</span>{{ selectionResult.strategy }}<span v-if="selectionResult.cacheHit" class="badge badge-success badge-sm ml-2">cache</span></div>
              <div><span class="font-semibold">最佳候选：</span>{{ selectionResult.selected ? selectionResult.selected.candidate.address : '系统解析' }}</div>
              <div v-if="selectionResult.selected?.latencyMs"><span class="font-semibold">延迟：</span>{{ selectionResult.selected.latencyMs }} ms</div>
              <div><span class="font-semibold">备选数量：</span>{{ selectionResult.alternatives.length }}</div>
              <div v-if="selectionResult.outcome"><span class="font-semibold">历史结果：</span>{{ selectionResult.outcome.success }} 成功 / {{ selectionResult.outcome.failure }} 失败</div>
            </div>
          </div>
        </section>
      </div>

      <section class="card bg-base-100 shadow-sm">
        <div class="card-body space-y-3">
          <h2 class="card-title text-lg">选择记录</h2>
          <div v-if="selectionHistory.length === 0" class="text-sm text-base-content/60">暂无选择记录。</div>
          <div v-else class="space-y-3">
            <div
              v-for="entry in selectionHistory"
              :key="entry.id"
              class="rounded-lg border border-base-200 p-3 space-y-2"
            >
              <div class="flex flex-wrap items-center justify-between text-sm font-medium">
                <span>{{ formatTimestamp(entry.timestamp) }}</span>
                <span>
                  {{ entry.host }}:{{ entry.port }} · {{ entry.strategy }}
                  <span v-if="entry.cacheHit" class="badge badge-success badge-sm ml-2">cache</span>
                </span>
              </div>
              <div class="overflow-x-auto">
                <table class="table table-xs">
                  <thead>
                    <tr>
                      <th>IP</th>
                      <th>来源</th>
                      <th>解析器</th>
                      <th>延迟 (ms)</th>
                      <th>角色</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr v-for="(candidate, idx) in entry.candidates" :key="`${entry.id}-${idx}`">
                      <td>{{ candidate.stat.candidate.address }}</td>
                      <td>{{ candidate.stat.sources.join(', ') || '-' }}</td>
                      <td>{{ candidate.stat.resolverMetadata?.join(', ') || '-' }}</td>
                      <td>{{ candidate.stat.latencyMs ?? '-' }}</td>
                      <td>
                        <span
                          class="badge badge-sm"
                          :class="candidate.role === 'best' ? 'badge-primary' : 'badge-outline'"
                        >
                          {{ candidate.role === 'best' ? '最佳' : '备选' }}
                        </span>
                      </td>
                    </tr>
                  </tbody>
                </table>
              </div>
            </div>
          </div>
        </div>
      </section>

      <section class="card bg-base-100 shadow-sm">
        <div class="card-body space-y-3">
          <h2 class="card-title text-lg">缓存快照</h2>
          <div v-if="cacheEmpty" class="text-sm text-base-content/60">暂无缓存条目，可通过预热或候选测试生成。</div>
          <div v-else class="overflow-x-auto">
            <table class="table table-zebra table-sm">
              <thead>
                <tr>
                  <th>域名</th>
                  <th>最佳候选</th>
                  <th>延迟 (ms)</th>
                  <th>到期时间</th>
                  <th>来源</th>
                  <th>成功/失败</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="entry in cacheEntries" :key="`${entry.host}:${entry.port}`">
                  <td>
                    <div class="font-semibold">{{ entry.host }}:{{ entry.port }}</div>
                    <div class="text-xs text-base-content/60">备选 {{ entry.alternatives.length }}</div>
                  </td>
                  <td>{{ entry.best ? entry.best.candidate.address : '系统 DNS' }}</td>
                  <td>{{ entry.best?.latencyMs ?? '-' }}</td>
                  <td>
                    <span v-if="entry.best?.expiresAtEpochMs">{{ formatTimestamp(entry.best.expiresAtEpochMs) }}</span>
                    <span v-else>-</span>
                  </td>
                  <td class="text-xs">
                    <template v-if="entry.best">
                      <div>{{ entry.best.sources.join(', ') || '-' }}</div>
                      <div v-if="entry.best.resolverMetadata?.length" class="text-base-content/60">
                        {{ entry.best.resolverMetadata.join(', ') }}
                      </div>
                    </template>
                    <span v-else>-</span>
                  </td>
                  <td class="text-xs">
                    <span v-if="entry.outcome">{{ entry.outcome.success }} / {{ entry.outcome.failure }}</span>
                    <span v-else>-</span>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
      </section>

      <section class="card bg-base-100 shadow-sm">
        <div class="card-body space-y-3">
          <h2 class="card-title text-lg">熔断 IP</h2>
          <div v-if="trippedIps.length === 0" class="text-sm text-base-content/60">当前没有被熔断的 IP。</div>
          <div v-else class="flex flex-wrap gap-2">
            <span v-for="ip in trippedIps" :key="ip" class="badge badge-outline">{{ ip }}</span>
          </div>
        </div>
      </section>
    </div>

    <div v-else-if="loading" class="flex items-center justify-center py-20 text-base-content/60">
      <span class="loading loading-spinner loading-lg mr-3"></span>
      正在加载 IP 池状态...
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue';
import type {
  DnsResolverConfig,
  DnsResolverPreset,
  DnsResolverProtocol,
  IpPoolFileConfig,
  IpPoolRuntimeConfig,
} from '../api/config';
import {
  clearIpPoolAutoDisabled,
  getIpPoolSnapshot,
  pickIpPoolBest,
  requestIpPoolRefresh,
  updateIpPoolConfig,
  type IpPoolSnapshot,
  type IpSelectionResult,
  type IpStat,
} from '../api/ip-pool';

interface PreheatDomainForm {
  host: string;
  portsText: string;
}

interface UserStaticForm {
  host: string;
  ip: string;
  portsText: string;
}

interface DnsResolverForm {
  label: string;
  protocol: DnsResolverProtocol;
  endpoint: string;
  portText: string;
  bootstrapText: string;
  sni: string;
  cacheSizeText: string;
  desc: string;
}

interface SelectionHistoryEntryCandidate {
  stat: IpStat;
  role: 'best' | 'alt';
}

interface SelectionHistoryEntry {
  id: number;
  timestamp: number;
  host: string;
  port: number;
  strategy: string;
  cacheHit: boolean;
  candidates: SelectionHistoryEntryCandidate[];
}

interface DnsPresetEntry {
  key: string;
  preset: DnsResolverPreset;
  protocol: string;
  enabled: boolean;
  disabled: boolean;
}

function inferPresetProtocol(preset: DnsResolverPreset): string {
  const type = preset.type?.toLowerCase();
  if (type === 'https' || type === 'doh') {
    return 'DoH';
  }
  if (type === 'tls' || type === 'dot') {
    return 'DoT';
  }
  if (type === 'udp') {
    return 'UDP';
  }
  if (preset.server.startsWith('tls://')) {
    return 'DoT';
  }
  if (preset.server.startsWith('https://')) {
    return 'DoH';
  }
  return 'DoH';
}

function createDefaultRuntime(): IpPoolRuntimeConfig {
  return {
    enabled: false,
    sources: {
      builtin: true,
      dns: true,
      history: true,
      userStatic: true,
      fallback: true,
    },
    dns: {
      useSystem: true,
      resolvers: [],
      presetCatalog: {},
      enabledPresets: [],
    },
    maxParallelProbes: 4,
    probeTimeoutMs: 1500,
    historyPath: null,
    cachePruneIntervalSecs: 60,
    maxCacheEntries: 256,
    singleflightTimeoutMs: 10_000,
    failureThreshold: 3,
    failureRateThreshold: 0.5,
    failureWindowSeconds: 60,
    minSamplesInWindow: 5,
    cooldownSeconds: 300,
    circuitBreakerEnabled: true,
  };
}

const loading = ref(false);
const saving = ref(false);
const loadError = ref<string | null>(null);
const saveError = ref<string | null>(null);
const refreshMessage = ref<string | null>(null);

const snapshot = ref<IpPoolSnapshot | null>(null);
const runtimeForm = reactive<IpPoolRuntimeConfig>(createDefaultRuntime());
const historyPathText = ref('');
const scoreTtlSeconds = ref(300);
const preheatDomainsForm = ref<PreheatDomainForm[]>([]);
const userStaticForm = ref<UserStaticForm[]>([]);
const dnsResolversForm = ref<DnsResolverForm[]>([]);
const blacklistText = ref('');
const whitelistText = ref('');
const disabledBuiltinText = ref('');

const testHost = ref('github.com');
const testPort = ref(443);
const testLoading = ref(false);
const testError = ref<string | null>(null);
const selectionResult = ref<IpSelectionResult | null>(null);
const selectionHistory = ref<SelectionHistoryEntry[]>([]);

const dnsPresetEntries = computed<DnsPresetEntry[]>(() => {
  const catalog = runtimeForm.dns?.presetCatalog ?? {};
  const enabledSet = new Set(runtimeForm.dns?.enabledPresets ?? []);
  return Object.entries(catalog)
    .map(([key, preset]) => ({
      key,
      preset: { ...preset },
      protocol: inferPresetProtocol(preset),
      enabled: enabledSet.has(key),
      disabled: preset.desc === '不可用',
    }))
    .sort((a, b) => a.key.localeCompare(b.key, 'zh-CN'));
});

const hasDnsPresets = computed(() => dnsPresetEntries.value.length > 0);

const cacheEntries = computed(() => snapshot.value?.cacheEntries ?? []);
const cacheEmpty = computed(() => cacheEntries.value.length === 0);
const trippedIps = computed(() => snapshot.value?.trippedIps ?? []);

const preheaterActive = computed(() => snapshot.value?.preheaterActive ?? false);

const lastUpdated = computed(() => {
  if (!snapshot.value) return '-';
  return new Date(snapshot.value.timestampMs).toLocaleString();
});

const autoDisabledInfo = computed(() => {
  const until = snapshot.value?.autoDisabledUntil ?? null;
  if (!until) return null;
  const now = Date.now();
  const remain = until - now;
  if (remain <= 0) {
    return { active: false, label: '冷却结束' };
  }
  const seconds = Math.ceil(remain / 1000);
  return {
    active: true,
    label: `禁用中，剩余 ${seconds}s` as const,
  };
});

const saveDisabled = computed(() => loading.value || saving.value);

onMounted(() => {
  loadSnapshot();
});

async function loadSnapshot() {
  loading.value = true;
  loadError.value = null;
  try {
    const data = await getIpPoolSnapshot();
    applySnapshot(data);
  } catch (err: any) {
    loadError.value = String(err);
  } finally {
    loading.value = false;
  }
}

function applySnapshot(data: IpPoolSnapshot) {
  snapshot.value = data;
  hydrateRuntimeForm(data.runtime);
  historyPathText.value = data.runtime.historyPath ?? '';
  scoreTtlSeconds.value = data.file.scoreTtlSeconds;
  preheatDomainsForm.value = data.file.preheatDomains.map((item) => ({
    host: item.host,
    portsText: item.ports.join(', '),
  }));
  userStaticForm.value = data.file.userStatic.map((item) => ({
    host: item.host,
    ip: item.ip,
    portsText: item.ports.join(', '),
  }));
  dnsResolversForm.value = data.runtime.dns.resolvers.map(toDnsResolverForm);
  blacklistText.value = data.file.blacklist.join('\n');
  whitelistText.value = data.file.whitelist.join('\n');
  disabledBuiltinText.value = data.file.disabledBuiltinPreheat.join('\n');
}

function hydrateRuntimeForm(source: IpPoolRuntimeConfig) {
  runtimeForm.enabled = source.enabled;
  runtimeForm.maxParallelProbes = source.maxParallelProbes;
  runtimeForm.probeTimeoutMs = source.probeTimeoutMs;
  runtimeForm.cachePruneIntervalSecs = source.cachePruneIntervalSecs;
  runtimeForm.maxCacheEntries = source.maxCacheEntries;
  runtimeForm.singleflightTimeoutMs = source.singleflightTimeoutMs;
  runtimeForm.failureThreshold = source.failureThreshold;
  runtimeForm.failureRateThreshold = source.failureRateThreshold;
  runtimeForm.failureWindowSeconds = source.failureWindowSeconds;
  runtimeForm.minSamplesInWindow = source.minSamplesInWindow;
  runtimeForm.cooldownSeconds = source.cooldownSeconds;
  runtimeForm.circuitBreakerEnabled = source.circuitBreakerEnabled;
  runtimeForm.sources.builtin = source.sources.builtin;
  runtimeForm.sources.dns = source.sources.dns;
  runtimeForm.sources.history = source.sources.history;
  runtimeForm.sources.userStatic = source.sources.userStatic;
  runtimeForm.sources.fallback = source.sources.fallback;
  runtimeForm.historyPath = source.historyPath ?? null;
  runtimeForm.dns.useSystem = source.dns.useSystem;
  runtimeForm.dns.resolvers = source.dns.resolvers.map((resolver) => ({ ...resolver }));
  runtimeForm.dns.presetCatalog = Object.fromEntries(
    Object.entries(source.dns.presetCatalog ?? {}).map(([key, preset]) => [
      key,
      { ...preset },
    ]),
  );
  runtimeForm.dns.enabledPresets = [...(source.dns.enabledPresets ?? [])];
  sanitizeEnabledPresets();
}

function sanitizeEnabledPresets() {
  const catalog = runtimeForm.dns?.presetCatalog ?? {};
  const allowed = new Set(Object.keys(catalog));
  runtimeForm.dns.enabledPresets = runtimeForm.dns.enabledPresets
    .filter((key, index, arr) => {
      if (!allowed.has(key)) {
        return false;
      }
      const preset = catalog[key];
      if (preset?.desc === '不可用') {
        return false;
      }
      return arr.indexOf(key) === index;
    });
}

function togglePreset(key: string, value: boolean) {
  const list = runtimeForm.dns.enabledPresets;
  if (value) {
    if (!list.includes(key)) {
      list.push(key);
    }
  } else {
    const idx = list.indexOf(key);
    if (idx >= 0) {
      list.splice(idx, 1);
    }
  }
  sanitizeEnabledPresets();
}

function addPreheatDomain() {
  preheatDomainsForm.value.push({ host: '', portsText: '443' });
}

function removePreheatDomain(index: number) {
  preheatDomainsForm.value.splice(index, 1);
}

function addUserStatic() {
  userStaticForm.value.push({ host: '', ip: '', portsText: '443' });
}

function removeUserStatic(index: number) {
  userStaticForm.value.splice(index, 1);
}

function toDnsResolverForm(resolver: DnsResolverConfig): DnsResolverForm {
  return {
    label: resolver.label,
    protocol: resolver.protocol,
    endpoint: resolver.endpoint,
    portText: resolver.port != null ? String(resolver.port) : '',
    bootstrapText: resolver.bootstrapIps.join(', '),
    sni: resolver.sni ?? '',
    cacheSizeText: resolver.cacheSize != null ? String(resolver.cacheSize) : '',
    desc: resolver.desc ?? '',
  };
}

function addDnsResolver() {
  dnsResolversForm.value.push({
    label: '',
    protocol: 'doh',
    endpoint: '',
    portText: '',
    bootstrapText: '',
    sni: '',
    cacheSizeText: '',
    desc: '',
  });
}

function removeDnsResolver(index: number) {
  dnsResolversForm.value.splice(index, 1);
}

function parsePorts(text: string): number[] {
  const nums = text
    .split(/[,\s]+/)
    .map((item) => parseInt(item, 10))
    .filter((value) => Number.isFinite(value) && value > 0 && value <= 65_535);
  const unique = Array.from(new Set(nums));
  return unique.length > 0 ? unique : [443];
}

function parseCidrList(text: string): string[] {
  return Array.from(
    new Set(
      text
        .split(/\r?\n|,/)
        .map((item) => item.trim())
        .filter(Boolean),
    ),
  );
}

function parseBootstrapList(text: string): string[] {
  return Array.from(
    new Set(
      text
        .split(/[\r?\n,\s]+/)
        .map((item) => item.trim())
        .filter(Boolean),
    ),
  );
}

function parseHostList(text: string): string[] {
  return Array.from(
    new Set(
      text
        .split(/\r?\n|,/)
        .map((item) => item.trim().toLowerCase())
        .filter(Boolean),
    ),
  );
}

function buildDnsResolvers(): DnsResolverConfig[] {
  return dnsResolversForm.value
    .map((item) => {
      const endpoint = item.endpoint.trim();
      if (!endpoint) {
        return undefined;
      }
      const portNumber = Number.parseInt(item.portText, 10);
      const port = Number.isFinite(portNumber) && portNumber > 0 && portNumber <= 65_535 ? portNumber : undefined;
      const bootstrapIps = parseBootstrapList(item.bootstrapText);
      const label = item.label.trim() || endpoint;
      const config: DnsResolverConfig = {
        label,
        protocol: item.protocol,
        endpoint,
        bootstrapIps,
      };
      if (port !== undefined) {
        config.port = port;
      }
      const sni = item.sni.trim();
      if (sni) {
        config.sni = sni;
      }
      const cacheNumber = Number.parseInt(item.cacheSizeText, 10);
      if (Number.isFinite(cacheNumber) && cacheNumber >= 0) {
        config.cacheSize = cacheNumber;
      }
      const desc = item.desc.trim();
      if (desc) {
        config.desc = desc;
      }
      return config;
    })
    .filter((config): config is DnsResolverConfig => config !== undefined);
}

function buildRuntimePayload(): IpPoolRuntimeConfig {
  const rate = Math.min(1, Math.max(0, Number(runtimeForm.failureRateThreshold) || 0));
  runtimeForm.historyPath = historyPathText.value.trim() ? historyPathText.value.trim() : null;
  sanitizeEnabledPresets();
  const presetCatalog = clonePresetCatalog();
  const enabledPresets = runtimeForm.dns.enabledPresets.filter((key, index, arr) => {
    const exists = Object.prototype.hasOwnProperty.call(presetCatalog, key);
    return exists && arr.indexOf(key) === index;
  });
  return {
    enabled: !!runtimeForm.enabled,
    sources: { ...runtimeForm.sources },
    dns: {
      useSystem: !!runtimeForm.dns.useSystem,
      resolvers: buildDnsResolvers(),
      presetCatalog,
      enabledPresets,
    },
    maxParallelProbes: Math.max(1, Math.floor(Number(runtimeForm.maxParallelProbes) || 1)),
    probeTimeoutMs: Math.max(100, Math.floor(Number(runtimeForm.probeTimeoutMs) || 100)),
    historyPath: runtimeForm.historyPath,
    cachePruneIntervalSecs: Math.max(5, Math.floor(Number(runtimeForm.cachePruneIntervalSecs) || 5)),
    maxCacheEntries: Math.max(0, Math.floor(Number(runtimeForm.maxCacheEntries) || 0)),
    singleflightTimeoutMs: Math.max(100, Math.floor(Number(runtimeForm.singleflightTimeoutMs) || 100)),
    failureThreshold: Math.max(1, Math.floor(Number(runtimeForm.failureThreshold) || 1)),
    failureRateThreshold: rate,
    failureWindowSeconds: Math.max(1, Math.floor(Number(runtimeForm.failureWindowSeconds) || 1)),
    minSamplesInWindow: Math.max(1, Math.floor(Number(runtimeForm.minSamplesInWindow) || 1)),
    cooldownSeconds: Math.max(0, Math.floor(Number(runtimeForm.cooldownSeconds) || 0)),
    circuitBreakerEnabled: !!runtimeForm.circuitBreakerEnabled,
  };
}

function clonePresetCatalog(): Record<string, DnsResolverPreset> {
  const catalog = runtimeForm.dns?.presetCatalog ?? {};
  const result: Record<string, DnsResolverPreset> = {};
  for (const [key, preset] of Object.entries(catalog)) {
    result[key] = {
      server: preset.server,
      type: preset.type,
      sni: preset.sni,
      cacheSize: preset.cacheSize,
      desc: preset.desc,
      forSNI: preset.forSNI,
    };
  }
  return result;
}

function buildFilePayload(): IpPoolFileConfig {
  return {
    preheatDomains: preheatDomainsForm.value
      .map((item) => ({
        host: item.host.trim(),
        ports: parsePorts(item.portsText),
      }))
      .filter((item) => item.host.length > 0),
    scoreTtlSeconds: Math.max(30, Math.floor(Number(scoreTtlSeconds.value) || 30)),
    userStatic: userStaticForm.value
      .map((item) => ({
        host: item.host.trim(),
        ip: item.ip.trim(),
        ports: parsePorts(item.portsText),
      }))
      .filter((item) => item.host.length > 0 && item.ip.length > 0),
    blacklist: parseCidrList(blacklistText.value),
    whitelist: parseCidrList(whitelistText.value),
    disabledBuiltinPreheat: parseHostList(disabledBuiltinText.value),
  };
}

async function handleSave() {
  saving.value = true;
  saveError.value = null;
  refreshMessage.value = null;
  try {
    const updated = await updateIpPoolConfig(buildRuntimePayload(), buildFilePayload());
    applySnapshot(updated);
    refreshMessage.value = '配置已保存并刷新 IP 池实例。';
  } catch (err: any) {
    saveError.value = String(err);
  } finally {
    saving.value = false;
  }
}

async function handleRefresh() {
  refreshMessage.value = null;
  try {
    const accepted = await requestIpPoolRefresh();
    refreshMessage.value = accepted ? '已请求预热线程立即刷新。' : '预热线程未运行或未启用。';
  } catch (err: any) {
    refreshMessage.value = `触发预热失败：${String(err)}`;
  }
}

async function handleClearAutoDisable() {
  refreshMessage.value = null;
  try {
    const cleared = await clearIpPoolAutoDisabled();
    refreshMessage.value = cleared ? '已清除自动禁用状态。' : '当前未处于禁用状态。';
  } catch (err: any) {
    refreshMessage.value = `操作失败：${String(err)}`;
  }
  await loadSnapshot();
}

function recordSelection(result: IpSelectionResult) {
  const timestamp = Date.now();
  const candidates: SelectionHistoryEntryCandidate[] = [];
  if (result.selected) {
    candidates.push({ stat: result.selected, role: 'best' });
  }
  for (const alt of result.alternatives) {
    candidates.push({ stat: alt, role: 'alt' });
  }
  if (candidates.length === 0) {
    return;
  }
  selectionHistory.value.unshift({
    id: timestamp,
    timestamp,
    host: result.host,
    port: result.port,
    strategy: result.strategy,
    cacheHit: result.cacheHit,
    candidates,
  });
  if (selectionHistory.value.length > 12) {
    selectionHistory.value.splice(12);
  }
}

async function handlePick() {
  testLoading.value = true;
  testError.value = null;
  selectionResult.value = null;
  try {
    const host = testHost.value.trim() || 'github.com';
    const port = Math.max(1, Math.min(65_535, Math.floor(Number(testPort.value) || 443)));
    const result = await pickIpPoolBest(host, port);
    selectionResult.value = result;
    recordSelection(result);
  } catch (err: any) {
    testError.value = String(err);
  } finally {
    testLoading.value = false;
  }
}

function formatTimestamp(value?: number | null): string {
  if (!value || value <= 0) return '-';
  return new Date(value).toLocaleString();
}
</script>

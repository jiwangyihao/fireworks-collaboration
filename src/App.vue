<script setup lang="ts">
import { RouterView } from "vue-router";
import { themeChange } from "theme-change";
import { computed, onMounted, provide, ref } from "vue";
import { UserInfo } from "./utils/github-auth.ts";
import GlobalErrors from "./components/GlobalErrors.vue";
import { useConfigStore } from "./stores/config";
import { storeToRefs } from "pinia";

onMounted(() => {
  themeChange(false);
});

const themeList = [
  "light",
  "dark",
  "cupcake",
  "bumblebee",
  "emerald",
  "corporate",
  "synthwave",
  "retro",
  "cyberpunk",
  "valentine",
  "halloween",
  "garden",
  "forest",
  "aqua",
  "lofi",
  "pastel",
  "fantasy",
  "wireframe",
  "black",
  "luxury",
  "dracula",
  "cmyk",
  "autumn",
  "business",
  "acid",
  "lemonade",
  "night",
  "coffee",
  "winter",
  "dim",
  "nord",
  "sunset",
  "caramellatte",
  "abyss",
  "silk",
];

const authenticated = ref(false);
provide("authenticated", authenticated);
const user = ref<UserInfo | null>(null);
provide("user", user);

const configStore = useConfigStore();
const { cfg: config } = storeToRefs(configStore);

const observabilityNavVisible = computed(() => {
  const cfg = config.value;
  if (!cfg) {
    return true;
  }
  const obs = cfg.observability;
  if (!obs) {
    return false;
  }
  if (!obs.enabled) {
    return false;
  }
  if (obs.uiEnabled === false) {
    return false;
  }
  return true;
});
</script>

<template>
  <header class="fixed p-3 flex w-full h-14 gap-4 justify-end z-10">
    <div class="flex-1 flex gap-2 items-center">
      <RouterLink to="/credentials" class="btn btn-sm btn-ghost gap-1.5">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <rect x="3" y="11" width="18" height="11" rx="2" ry="2"></rect>
          <path d="M7 11V7a5 5 0 0 1 10 0v4"></path>
        </svg>
        <span class="hidden sm:inline">凭据管理</span>
      </RouterLink>
      <RouterLink to="/workspace" class="btn btn-sm btn-ghost gap-1.5">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
          <path d="M3 9h18"></path>
          <path d="M9 21V9"></path>
        </svg>
        <span class="hidden sm:inline">工作区</span>
      </RouterLink>
      <RouterLink to="/git" class="btn btn-sm btn-ghost gap-1.5">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="currentColor"
        >
          <path
            d="M23.546 10.93L13.067.452c-.604-.603-1.582-.603-2.188 0L8.708 2.627l2.76 2.76c.645-.215 1.379-.07 1.889.441.516.515.658 1.258.438 1.9l2.658 2.66c.645-.223 1.387-.078 1.9.435.721.72.721 1.884 0 2.604-.719.719-1.881.719-2.6 0-.539-.541-.674-1.337-.404-1.996L12.86 8.955v6.525c.176.086.342.203.488.348.713.721.713 1.883 0 2.6-.719.721-1.889.721-2.609 0-.719-.719-.719-1.879 0-2.598.182-.18.387-.316.605-.406V8.835c-.217-.091-.424-.222-.6-.401-.545-.545-.676-1.342-.396-2.009L7.636 3.7.45 10.881c-.6.605-.6 1.584 0 2.189l10.48 10.477c.604.604 1.582.604 2.186 0l10.43-10.43c.605-.603.605-1.582 0-2.188"
          ></path>
        </svg>
        <span class="hidden sm:inline">Git 面板</span>
      </RouterLink>
      <RouterLink
        v-if="observabilityNavVisible"
        to="/observability"
        class="btn btn-sm btn-ghost gap-1.5"
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <path d="M4 19V9"></path>
          <path d="M8 19V5"></path>
          <path d="M12 19v-7"></path>
          <path d="M16 19v-3"></path>
          <path d="M20 19V8"></path>
        </svg>
        <span class="hidden sm:inline">可观测性</span>
      </RouterLink>
      <RouterLink to="/http-tester" class="btn btn-sm btn-ghost gap-1.5">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <circle cx="12" cy="12" r="10"></circle>
          <line x1="2" y1="12" x2="22" y2="12"></line>
          <path
            d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"
          ></path>
        </svg>
        <span class="hidden sm:inline">HTTP 测试</span>
      </RouterLink>
    </div>
    <div v-if="user" class="flex items-center gap-2">
      <div class="avatar">
        <div class="h-7 w-7 rounded-full">
          <img :src="user.avatar_url" />
        </div>
      </div>
      <span class="font-bold text-sm">{{ user.name }}</span>
    </div>
    <button
      class="btn group btn-sm gap-1.5 px-1.5 btn-ghost"
      popovertarget="theme-chooser"
      style="anchor-name: --anchor-theme-chooser"
    >
      <div
        class="bg-base-100 group-hover:border-base-content/20 border-base-content/10 grid shrink-0 grid-cols-2 gap-0.5 rounded-md border p-1 transition-colors"
      >
        <div class="bg-base-content size-1 rounded-full"></div>
        <div class="bg-primary size-1 rounded-full"></div>
        <div class="bg-secondary size-1 rounded-full"></div>
        <div class="bg-accent size-1 rounded-full"></div>
      </div>
      <svg
        width="12px"
        height="12px"
        class="mt-px hidden size-2 fill-current opacity-60 sm:inline-block"
        xmlns="http://www.w3.org/2000/svg"
        viewBox="0 0 2048 2048"
      >
        <path
          d="M1799 349l242 241-1017 1017L7 590l242-241 775 775 775-775z"
        ></path>
      </svg>
    </button>
    <ul
      class="dropdown dropdown-end menu w-48 max-h-[calc(100vh-32*var(--spacing))] rounded-box bg-base-100 shadow-sm"
      popover
      id="theme-chooser"
      style="position-anchor: --anchor-theme-chooser"
    >
      <li class="menu-title text-xs">主题</li>
      <li v-for="theme in themeList" :key="theme">
        <button
          class="gap-3 px-2 [&_svg]:visible"
          :data-set-theme="theme"
          data-act-class="[&_svg]:visible"
        >
          <div
            :data-theme="theme"
            class="bg-base-100 grid shrink-0 grid-cols-2 gap-0.5 rounded-md p-1 shadow-sm"
          >
            <div class="bg-base-content size-1 rounded-full"></div>
            <div class="bg-primary size-1 rounded-full"></div>
            <div class="bg-secondary size-1 rounded-full"></div>
            <div class="bg-accent size-1 rounded-full"></div>
          </div>
          <div class="flex-1 truncate">{{ theme }}</div>
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="currentColor"
            class="invisible h-3 w-3 shrink-0"
          >
            <path
              d="M20.285 2l-11.285 11.567-5.286-5.011-3.714 3.716 9 8.728 15-15.285z"
            ></path>
          </svg>
        </button>
      </li>
    </ul>
  </header>
  <RouterView />
  <GlobalErrors />
</template>

<!--suppress CssUnusedSymbol -->
<style>
@import "tailwindcss";
@plugin "daisyui" {
  themes: all;
}
@plugin "@tailwindcss/typography";

/*noinspection CssInvalidPropertyValue*/
@plugin "daisyui/theme" {
  name: "light";
  --radius-selector: 0.5rem;
  --radius-field: 0.5rem;
  --radius-box: 1rem;
  --size-selector: 0.25rem;
  --size-field: 0.25rem;
  --border: 2px;
  --noise: 1;
}

/*noinspection CssInvalidPropertyValue*/
@plugin "daisyui/theme" {
  name: "dark";
  --radius-selector: 0.5rem;
  --radius-field: 0.5rem;
  --radius-box: 1rem;
  --size-selector: 0.25rem;
  --size-field: 0.25rem;
  --border: 2px;
  --noise: 1;
}

@custom-variant dark (&:where(
  [data-theme=dark],
  [data-theme=dark] *,
  [data-theme=synthwave],
  [data-theme=synthwave] *,
  [data-theme=halloween],
  [data-theme=halloween] *,
  [data-theme=forest],
  [data-theme=forest] *,
  [data-theme=black],
  [data-theme=black] *,
  [data-theme=luxury],
  [data-theme=luxury] *,
  [data-theme=dracula],
  [data-theme=dracula] *,
  [data-theme=business],
  [data-theme=business] *,
  [data-theme=night],
  [data-theme=night] *,
  [data-theme=coffee],
  [data-theme=coffee] *
  [data-theme=dim],
  [data-theme=dim] *,
  [data-theme=sunset],
  [data-theme=sunset] *,
  [data-theme=abyss],
  [data-theme=abyss] *,
));

@utility page {
  @apply w-full min-w-full h-full prose flex flex-col p-4;
}

@utility vertical-lr {
  writing-mode: vertical-lr;
}

#app {
  width: 100%;
  height: 100vh;
}

.list-move, /* 对移动中的元素应用的过渡 */
.list-enter-active,
.list-leave-active {
  transition: all 0.5s ease;
}

.list-enter-from,
.list-leave-to {
  opacity: 0;
  transform: translateX(30px);
}

/* 确保将离开的元素从布局流中删除
  以便能够正确地计算移动的动画。 */
.list-leave-active {
  position: absolute;
}
</style>

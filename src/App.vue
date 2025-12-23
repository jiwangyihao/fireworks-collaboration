<script setup lang="ts">
import { RouterView } from "vue-router";
import { themeChange } from "theme-change";
import { onMounted, provide, ref } from "vue";
import { UserInfo } from "./utils/github-auth.ts";
// import GlobalErrors from "./components/GlobalErrors.vue";
import GlobalToast from "./components/GlobalToast.vue";

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
</script>

<template>
  <!-- 右上角固定的全局导航区域 -->
  <header class="fixed top-0 right-0 p-3 flex h-14 gap-3 items-center z-10">
    <!-- 开发调试链接 -->
    <RouterLink to="/dev-tools" class="btn btn-sm btn-ghost gap-1.5">
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
        <path
          d="m11 7-7.64 7.64c-.61.61-.94 1.44-.94 2.31V21h3.05c.87 0 1.7-.34 2.31-.95L16 13"
        ></path>
        <path
          d="m14 4 5.06 5.06c.59.59.59 1.54 0 2.12l-4 4c-.59.59-1.54.59-2.12 0L7.94 9"
        ></path>
      </svg>
      <span class="hidden sm:inline">开发调试</span>
    </RouterLink>
    <!-- 用户信息 -->
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
  <!-- <GlobalErrors /> -->
  <GlobalToast />
</template>

<style>
#app {
  width: 100%;
  height: 100vh;
}
.list-move,
.list-enter-active,
.list-leave-active {
  transition: all 0.5s ease;
}
.list-enter-from,
.list-leave-to {
  opacity: 0;
  transform: translateX(30px);
}
.list-leave-active {
  position: absolute;
}
</style>

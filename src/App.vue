<script setup lang="ts">
import { RouterView } from "vue-router";
import { themeChange } from "theme-change";
import { onMounted } from "vue";

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
</script>

<template>
  <header class="fixed p-2 flex w-full justify-end z-10">
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
</template>

<!--suppress CssUnusedSymbol -->
<style>
@import "tailwindcss";
@plugin "daisyui" {
  themes: all;
}
@plugin "@tailwindcss/typography";

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

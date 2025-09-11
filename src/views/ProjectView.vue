<script setup lang="ts"></script>

<template>
  <main class="page">
    <h2 class="mt-0!">项目管理</h2>
    <div class="flex flex-1 w-full not-prose">
      <div class="card card-dash grow p-4">
        <div class="indicator w-auto">
          <span
            class="indicator-item badge badge-soft bg-base-100 indicator-top indicator-start ml-4"
            style="--indicator-x: 0"
          >
            目标仓库
          </span>
          <div class="card card-border bg-base-100 shadow-sm">
            <div class="card-body">
              <h2 class="card-title">Card Title</h2>
              <p>
                A card component has a figure, a body part, and inside body
                there are title and actions parts
              </p>
              <div class="card-actions justify-end">
                <button class="btn btn-primary">Buy Now</button>
              </div>
            </div>
          </div>
        </div>
      </div>
      <div class="divider divider-horizontal">
        <span class="vertical-lr">远端 \ 本地</span>
      </div>
      <div class="card card-dash grow p-4">
        <div class="indicator w-auto">
          <span class="indicator-item badge">Required</span>
          <div class="card bg-base-100 shadow-sm">
            <div class="card-body">
              <h2 class="card-title">Card Title</h2>
              <p>
                A card component has a figure, a body part, and inside body
                there are title and actions parts
              </p>
              <div class="card-actions justify-end">
                <button class="btn btn-primary">Buy Now</button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </main>
</template>

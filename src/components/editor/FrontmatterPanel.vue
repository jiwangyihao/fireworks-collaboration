<!-- src/components/editor/FrontmatterPanel.vue -->
<script setup lang="ts">
/**
 * FrontmatterPanel - 文档元数据动态编辑面板 (E2.5 重构版)
 *
 * 核心特性：
 * - 只显示已配置的 frontmatter 项
 * - 通过 "+" 按钮和分组下拉菜单添加新配置
 * - 每个配置项可单独删除
 * - 根据字段类型渲染对应输入控件
 */
import { ref, computed, watch, onMounted, onUnmounted } from "vue";
import { useDocumentStore } from "@/stores/document";
import BaseIcon from "@/components/BaseIcon.vue";

// ========== 类型定义 ==========
interface FieldConfig {
  key: string;
  label: string;
  description?: string;
  type: "text" | "textarea" | "select" | "toggle";
  options?: { value: any; label: string }[];
  placeholder?: string;
  group: "basic" | "layout" | "navigation" | "meta" | "sidebar" | "advanced";
}

interface Frontmatter {
  [key: string]: any;
}

interface GroupInfo {
  label: string;
  icon: string;
  fields: FieldConfig[];
}

// ========== 字段配置注册表 ==========
const FIELD_REGISTRY: FieldConfig[] = [
  // 基础信息
  {
    key: "title",
    label: "页面标题",
    type: "text",
    group: "basic",
    description: "设置当前页面的标题，将显示在浏览器标签页和侧边栏导航中。",
    placeholder: "页面标题",
  },
  {
    key: "titleTemplate",
    label: "标题后缀",
    type: "text",
    group: "basic",
    description:
      "自定义标题后缀，覆盖默认的站点标题显示。例如：':title | 站点名称'。",
    placeholder: "| 站点名称",
  },
  {
    key: "description",
    label: "页面描述",
    type: "textarea",
    group: "basic",
    description: "页面的简短描述，用于搜索引擎优化 (SEO) 和社交分享预览。",
    placeholder: "简短描述页面内容...",
  },

  // 布局
  {
    key: "layout",
    label: "布局",
    type: "select",
    group: "layout",
    description: "选择页面的整体布局结构。",
    options: [
      { value: "", label: "默认 (doc)" },
      { value: "doc", label: "文档页面" },
      { value: "home", label: "首页" },
      { value: "page", label: "空白页面" },
    ],
  },
  {
    key: "outline",
    label: "大纲",
    type: "select",
    group: "layout",
    description: "控制右侧大纲目录的显示层级深度。",
    options: [
      { value: 2, label: "H2 (默认)" },
      { value: 3, label: "H2-H3" },
      { value: "deep", label: "全部层级" },
      { value: false, label: "隐藏" },
    ],
  },
  {
    key: "aside",
    label: "右侧栏",
    type: "select",
    group: "layout",
    description: "控制右侧栏（大纲）的显示位置或隐藏。",
    options: [
      { value: true, label: "右侧" },
      { value: "left", label: "左侧" },
      { value: false, label: "隐藏" },
    ],
  },
  {
    key: "navbar",
    label: "导航栏",
    type: "toggle",
    group: "layout",
    description: "是否显示顶部导航栏区域。",
  },
  {
    key: "sidebar",
    label: "侧边栏",
    type: "toggle",
    group: "layout",
    description: "是否显示左侧侧边栏导航。",
  },
  {
    key: "footer",
    label: "页脚",
    type: "toggle",
    group: "layout",
    description: "是否显示页面底部的页脚区域。",
  },
  {
    key: "lastUpdated",
    label: "更新时间",
    type: "toggle",
    group: "layout",
    description: "是否显示「最后更新时间」信息。",
  },
  {
    key: "editLink",
    label: "编辑链接",
    type: "toggle",
    group: "layout",
    description: "是否显示指向 Git 仓库的「编辑此页」链接。",
  },

  // Sidebar (vitepress-sidebar)
  {
    key: "order",
    label: "排序优先级",
    type: "text",
    group: "sidebar",
    description: "侧边栏菜单的排序优先级（数字），数字越小越靠前。",
    placeholder: "0",
  },
  {
    key: "date",
    label: "发布日期",
    type: "text",
    group: "sidebar",
    description: "文章的发布或最后更新日期 (YYYY-MM-DD)，用于侧边栏排序。",
    placeholder: "YYYY-MM-DD",
  },
  {
    key: "exclude",
    label: "从侧边栏排除",
    type: "toggle",
    group: "sidebar",
    description: "将当前页面从自动生成的侧边栏菜单中排除。",
  },

  // 高级
  {
    key: "pageClass",
    label: "自定义样式类",
    type: "text",
    group: "advanced",
    description: "为当前页面的根容器添加额外的 CSS 类名，用于自定义样式。",
    placeholder: "custom-page-class",
  },
];

const GROUP_META: Record<string, { label: string; icon: string }> = {
  basic: { label: "基础信息", icon: "ph:text-t" },
  layout: { label: "布局与显示", icon: "ph:layout" },
  sidebar: { label: "侧边栏配置", icon: "ph:sidebar" },
  advanced: { label: "高级", icon: "ph:gear" },
};

// ========== Props & Emits ==========
const props = defineProps<{
  isOpen?: boolean;
}>();

const emit = defineEmits<{
  toggle: [];
}>();

// ========== State ==========
// ========== State ==========
const docStore = useDocumentStore();
const frontmatter = ref<Frontmatter>({});
const isAddDropdownOpen = ref(false);
const addButtonRef = ref<HTMLButtonElement | null>(null);
const menuRef = ref<HTMLUListElement | null>(null);
const menuPosition = ref<{ top: number; left: number } | null>(null);

// Select Dropdown State
const activeSelectKey = ref<string | null>(null);
const selectMenuRef = ref<HTMLUListElement | null>(null);
const selectMenuPosition = ref<{
  top: number;
  left: number;
  width: number;
} | null>(null);

// ========== 计算属性 ==========

// 当前已配置的字段（只包含注册表中定义的）
const activeFields = computed(() =>
  Object.keys(frontmatter.value).filter((key) =>
    FIELD_REGISTRY.some((f) => f.key === key)
  )
);

// 可添加的字段（尚未配置的）
const availableFields = computed(() =>
  FIELD_REGISTRY.filter((f) => !activeFields.value.includes(f.key))
);

// 按分组组织可添加字段
const availableFieldsByGroup = computed(() => {
  const groups: Record<string, GroupInfo> = {};

  for (const field of availableFields.value) {
    if (!groups[field.group]) {
      const meta = GROUP_META[field.group];
      groups[field.group] = {
        label: meta.label,
        icon: meta.icon,
        fields: [],
      };
    }
    groups[field.group].fields.push(field);
  }

  // 按预定义顺序返回
  // 按预定义顺序返回
  // 按预定义顺序返回
  const order = ["basic", "layout", "sidebar", "advanced"];
  return order
    .filter((g) => groups[g]?.fields.length > 0)
    .map((g) => ({ key: g, ...groups[g] }));
});

// ========== 方法 ==========

// 获取字段配置
function getFieldConfig(key: string): FieldConfig | undefined {
  return FIELD_REGISTRY.find((f) => f.key === key);
}

// 添加字段
function addField(field: FieldConfig) {
  if (field.type === "toggle") {
    frontmatter.value[field.key] = true;
  } else if (field.type === "select" && field.options?.length) {
    frontmatter.value[field.key] = field.options[0].value;
  } else {
    frontmatter.value[field.key] = "";
  }
  isAddDropdownOpen.value = false;
  updateFrontmatter();
}

// 移除字段
function removeField(key: string) {
  delete frontmatter.value[key];
  updateFrontmatter();
}

// 更新 frontmatter 到 store
// 注意：不过滤空字符串，让字段能正常显示；空值将在保存时清理
function updateFrontmatter() {
  const cleaned: Frontmatter = {};
  for (const [key, value] of Object.entries(frontmatter.value)) {
    // 只过滤 undefined 和 null，保留空字符串以便 UI 显示
    if (value !== undefined && value !== null) {
      cleaned[key] = value;
    }
  }
  docStore.updateFrontmatter(cleaned);
}

// 切换添加菜单
function toggleAddDropdown() {
  if (!isAddDropdownOpen.value && addButtonRef.value) {
    // 计算按钮位置，设置菜单位置
    const rect = addButtonRef.value.getBoundingClientRect();
    menuPosition.value = {
      top: rect.bottom + 4, // 按钮下方 4px
      left: rect.left,
    };
  }
  isAddDropdownOpen.value = !isAddDropdownOpen.value;
  activeSelectKey.value = null; // 关闭其他菜单
}

// 打开选择菜单
function openSelectMenu(key: string, event: MouseEvent) {
  const target = event.currentTarget as HTMLElement;
  const rect = target.getBoundingClientRect();
  selectMenuPosition.value = {
    top: rect.bottom + 4,
    left: rect.left,
    width: rect.width,
  };
  activeSelectKey.value = key;
  isAddDropdownOpen.value = false; // 关闭添加菜单
}

// 处理选项选择
function handleOptionSelect(value: any) {
  if (activeSelectKey.value) {
    frontmatter.value[activeSelectKey.value] = value;
    updateFrontmatter();
    activeSelectKey.value = null;
  }
}

// 点击外部关闭下拉菜单
function handleClickOutside(event: MouseEvent) {
  const target = event.target as Node;

  // 处理添加菜单关闭
  const isClickOnAddButton = addButtonRef.value?.contains(target);
  const isClickOnAddMenu = menuRef.value?.contains(target);
  if (!isClickOnAddButton && !isClickOnAddMenu) {
    isAddDropdownOpen.value = false;
  }

  // 处理选择菜单关闭
  const isClickOnSelectMenu = selectMenuRef.value?.contains(target);
  // 注意：触发按钮点击由 openSelectMenu 处理，不需要在这里判断，
  // 但为了防止点击触发按钮时先关闭后打开导致闪烁，这里通常不需要特别处理触发器，
  // 因为 openSelectMenu 会重新设置 activeSelectKey。
  // 不过如果点的是同一个触发器，最好不要 toggle，而是保持打开或关闭。
  // 这里简化处理：只要不是点击在菜单上，且 activeSelectKey不为空，就关闭。
  // 对于触发按钮的点击，事件冒泡会先触发 clickOutside 吗？通常不会，clickOutside 是 document 级的。
  // 为了安全，我们可以判断是否点击了 "select-trigger" class 的元素？或者更简单的，在打开逻辑里 stopPropagation
  if (!isClickOnSelectMenu && activeSelectKey.value !== null) {
    // 检查是否点击了触发器（通过 openSelectMenu stopPropagation 可以避免这里误关）
    // 但简单的做法是：让 openSelectMenu 使用 @click.stop
    activeSelectKey.value = null;
  }
}

// ========== 生命周期 ==========

// 同步 store 到本地状态
watch(
  () => docStore.currentFrontmatter,
  (fm) => {
    if (fm) {
      frontmatter.value = { ...fm };
    }
  },
  { immediate: true, deep: true }
);

onMounted(() => {
  document.addEventListener("click", handleClickOutside);
});

onUnmounted(() => {
  document.removeEventListener("click", handleClickOutside);
});

// 计算当前文件名（用于 title placeholder）
const selectedFileName = computed(() => {
  if (!docStore.selectedPath) return "";
  const parts = docStore.selectedPath.split(/[/\\]/);
  const filename = parts[parts.length - 1];
  return filename.replace(/\.md$/, "");
});
</script>

<template>
  <Transition name="slide-down">
    <div
      v-if="isOpen"
      class="frontmatter-panel z-[40] border-b border-base-300 bg-base-200/50 backdrop-blur-sm"
    >
      <!-- Header -->
      <div
        class="flex items-center gap-2 px-4 py-2 text-sm font-medium text-base-content/80 border-b border-base-content/5 bg-base-100/50"
      >
        <BaseIcon icon="ph--gear-six" size="sm" class="text-primary" />
        Frontmatter 配置
      </div>

      <!-- Content -->
      <div class="p-4 max-h-[60vh] overflow-y-auto custom-scrollbar">
        <!-- Fields Grid (always show, includes add button) -->
        <div class="grid grid-cols-1 gap-3">
          <!-- Active Fields -->
          <template v-for="key in activeFields" :key="key">
            <div
              v-if="getFieldConfig(key)"
              class="form-control bg-base-100 p-3 rounded-md border border-base-200 relative group"
            >
              <!-- Delete Button -->
              <button
                class="btn btn-ghost btn-xs btn-circle absolute -top-1.5 -right-1.5 opacity-0 group-hover:opacity-100 transition-opacity bg-base-100 border border-base-300 shadow-sm"
                @click="removeField(key)"
                title="移除配置"
              >
                <BaseIcon icon="ph:x" size="xs" />
              </button>

              <!-- Label -->
              <label
                class="label py-1 h-auto min-h-0 flex flex-col items-start gap-0.5"
              >
                <div class="flex items-center gap-2 w-full">
                  <span class="label-text text-xs font-medium">
                    {{ getFieldConfig(key)?.label }}
                    <span class="opacity-40 font-normal">({{ key }})</span>
                  </span>
                </div>
                <span
                  v-if="getFieldConfig(key)?.description"
                  class="text-[10px] text-base-content/50 leading-tight"
                >
                  {{ getFieldConfig(key)?.description }}
                </span>
              </label>

              <!-- Text Input -->
              <input
                v-if="getFieldConfig(key)?.type === 'text'"
                v-model="frontmatter[key]"
                type="text"
                class="input input-bordered input-xs w-full"
                :placeholder="
                  getFieldConfig(key)?.placeholder ||
                  (key === 'title' ? selectedFileName : '')
                "
                @blur="updateFrontmatter"
              />

              <!-- Textarea -->
              <textarea
                v-else-if="getFieldConfig(key)?.type === 'textarea'"
                v-model="frontmatter[key]"
                class="textarea textarea-bordered textarea-xs w-full h-16 min-h-[4rem] py-1 leading-normal"
                :placeholder="getFieldConfig(key)?.placeholder"
                @blur="updateFrontmatter"
              />

              <!-- Custom Select Dropdown Trigger -->
              <button
                v-else-if="getFieldConfig(key)?.type === 'select'"
                class="btn btn-xs btn-outline w-full justify-between font-normal bg-base-100 hover:bg-base-200 border-base-300 text-base-content"
                @click.stop="openSelectMenu(key, $event)"
              >
                <span class="truncate">{{
                  getFieldConfig(key)?.options?.find(
                    (opt) => opt.value === frontmatter[key]
                  )?.label || frontmatter[key]
                }}</span>
                <BaseIcon icon="ph:caret-down" size="xs" class="opacity-50" />
              </button>

              <!-- Toggle -->
              <div
                v-else-if="getFieldConfig(key)?.type === 'toggle'"
                class="flex items-center h-6 px-2 rounded border border-base-300 bg-base-200/50"
              >
                <input
                  type="checkbox"
                  v-model="frontmatter[key]"
                  class="toggle toggle-xs toggle-primary"
                  @change="updateFrontmatter"
                />
                <span class="ml-2 text-[10px] opacity-70">{{
                  frontmatter[key] ? "启用" : "禁用"
                }}</span>
              </div>
            </div>
          </template>

          <!-- Add Field Card (always visible at the end) -->
          <button
            v-if="availableFields.length > 0"
            ref="addButtonRef"
            class="flex flex-col items-center justify-center w-full h-full min-h-[4.5rem] p-3 rounded-md border-2 border-dashed border-base-300 hover:border-primary/50 hover:bg-primary/5 text-base-content/40 hover:text-primary transition-all cursor-pointer"
            @click.stop="toggleAddDropdown"
          >
            <BaseIcon icon="ph--plus" size="sm" class="mb-1" />
            <span class="text-xs font-medium">添加配置项</span>
          </button>
        </div>
      </div>
    </div>
  </Transition>

  <!-- Teleport: Add Field Dropdown Menu -->
  <Teleport to="body">
    <ul
      v-if="
        isAddDropdownOpen && availableFieldsByGroup.length > 0 && menuPosition
      "
      ref="menuRef"
      class="fixed z-[99999] menu menu-xs bg-base-100 rounded-xl w-48 shadow-xl border border-base-content/10 p-2 gap-1"
      :style="{ top: menuPosition.top + 'px', left: menuPosition.left + 'px' }"
    >
      <li v-for="group in availableFieldsByGroup" :key="group.key">
        <!-- Group with Submenu -->
        <div
          class="dropdown dropdown-hover dropdown-right dropdown-center p-0 !bg-transparent !border-none"
        >
          <div
            tabindex="0"
            role="button"
            class="flex items-center gap-2 w-full py-1.5 px-2 rounded-md border border-transparent hover:border-base-content/20 hover:bg-base-200 transition-all"
          >
            <BaseIcon :icon="group.icon" size="sm" class="opacity-60" />
            <span class="flex-1 text-left font-medium">{{ group.label }}</span>
            <BaseIcon icon="ph:caret-right" size="xs" class="opacity-50" />
          </div>
          <!-- Field Items (use -ml-1 + pl-2 to create overlap bridge) -->
          <ul
            tabindex="0"
            class="dropdown-content z-[100000] menu menu-xs bg-base-100 rounded-xl w-56 shadow-xl border border-base-content/10 -ml-1 pl-2 p-1.5 gap-1 overflow-hidden"
          >
            <li
              class="w-full overflow-hidden"
              v-for="field in group.fields"
              :key="field.key"
            >
              <a
                class="flex flex-col items-start gap-0.5 py-2 px-3 rounded-md border border-transparent hover:border-primary/30 hover:bg-primary/5 transition-all w-full overflow-hidden"
                @click.stop="addField(field)"
              >
                <span class="font-medium text-base-content">{{
                  field.label
                }}</span>
                <span
                  v-if="field.description"
                  class="text-[10px] opacity-60 leading-tight truncate w-full"
                  >{{ field.description }}</span
                >
              </a>
            </li>
          </ul>
        </div>
      </li>
    </ul>

    <!-- Shared Select Dropdown Menu -->
    <ul
      v-if="activeSelectKey && selectMenuPosition"
      ref="selectMenuRef"
      class="fixed z-[99999] menu menu-xs bg-base-100 rounded-xl shadow-xl border border-base-content/10 p-2 gap-1 overflow-y-auto max-h-60"
      :style="{
        top: selectMenuPosition.top + 'px',
        left: selectMenuPosition.left + 'px',
        width: selectMenuPosition.width + 'px',
      }"
    >
      <li
        v-for="opt in getFieldConfig(activeSelectKey)?.options || []"
        :key="String(opt.value)"
      >
        <a
          class="flex items-center gap-2 py-1.5 px-2 rounded-md border border-transparent transition-all"
          :class="[
            frontmatter[activeSelectKey] === opt.value
              ? '!border-primary bg-primary/5 text-primary font-medium'
              : 'hover:border-base-content/20 hover:bg-base-200 text-base-content',
          ]"
          @click.stop="handleOptionSelect(opt.value)"
        >
          {{ opt.label }}
          <BaseIcon
            v-if="frontmatter[activeSelectKey] === opt.value"
            icon="ph:check"
            size="xs"
            class="ml-auto"
          />
        </a>
      </li>
    </ul>
  </Teleport>
</template>

<style scoped>
/* Slide-down animation */
.slide-down-enter-active,
.slide-down-leave-active {
  transition: all 0.3s cubic-bezier(0.16, 1, 0.3, 1);
  overflow: hidden;
  max-height: 80vh;
  opacity: 1;
}

.slide-down-enter-from,
.slide-down-leave-to {
  opacity: 0;
  transform: translateY(-8px);
  max-height: 0;
}

/* Custom Scrollbar for the panel content */
.custom-scrollbar::-webkit-scrollbar {
  width: 4px;
}
.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}
.custom-scrollbar::-webkit-scrollbar-thumb {
  background-color: oklch(var(--bc) / 0.1);
  border-radius: 4px;
}
.custom-scrollbar::-webkit-scrollbar-thumb:hover {
  background-color: oklch(var(--bc) / 0.2);
}
</style>

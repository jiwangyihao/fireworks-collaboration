<template>
  <div class="card bg-base-100 shadow-sm">
    <div class="card-body">
      <h4 class="card-title text-base">
        {{ editMode ? "编辑凭证" : "添加凭证" }}
      </h4>

      <!-- Error Alert -->
      <div v-if="error" class="alert alert-error alert-sm">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          class="stroke-current shrink-0 h-5 w-5"
          fill="none"
          viewBox="0 0 24 24"
        >
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M10 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2m7-2a9 9 0 11-18 0 9 9 0 0118 0z"
          />
        </svg>
        <span class="text-sm">{{ error }}</span>
      </div>

      <div class="form-control">
        <label class="label" for="host">
          <span class="label-text"
            >服务地址 <span class="text-error">*</span></span
          >
        </label>
        <input
          id="host"
          v-model="form.host"
          type="text"
          placeholder="例如: github.com"
          class="input input-bordered input-sm"
          :disabled="editMode"
          @input="error = null"
        />
      </div>

      <div class="form-control">
        <label class="label" for="username">
          <span class="label-text"
            >用户名 <span class="text-error">*</span></span
          >
        </label>
        <input
          id="username"
          v-model="form.username"
          type="text"
          placeholder="Git 用户名"
          class="input input-bordered input-sm"
          :disabled="editMode"
          @input="error = null"
        />
      </div>

      <div class="form-control">
        <label class="label" for="password">
          <span class="label-text"
            >密码/令牌 <span class="text-error">*</span></span
          >
        </label>
        <input
          id="password"
          v-model="form.passwordOrToken"
          type="password"
          placeholder="Personal Access Token 或密码"
          class="input input-bordered input-sm"
          @input="error = null"
        />
      </div>

      <div class="form-control">
        <label class="label" for="expires">
          <span class="label-text">过期时间 (天)</span>
          <span class="label-text-alt">留空表示永不过期</span>
        </label>
        <input
          id="expires"
          v-model.number="form.expiresInDays"
          type="number"
          placeholder="例如: 90"
          class="input input-bordered input-sm"
          min="1"
          @input="error = null"
        />
      </div>

      <div class="card-actions justify-end mt-4">
        <button
          class="btn btn-sm btn-ghost"
          @click="cancel"
          :disabled="loading"
        >
          取消
        </button>
        <button
          class="btn btn-sm btn-primary"
          @click="submit"
          :disabled="!canSubmit || loading"
        >
          <span
            v-if="loading"
            class="loading loading-spinner loading-xs"
          ></span>
          {{ loading ? "保存中..." : "保存" }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from "vue";
import { useCredentialStore } from "../../stores/credential";

const props = defineProps<{
  editMode?: boolean;
  initialHost?: string;
  initialUsername?: string;
}>();

const emit = defineEmits<{
  success: [];
  cancel: [];
}>();

const credentialStore = useCredentialStore();

const form = ref({
  host: props.initialHost || "",
  username: props.initialUsername || "",
  passwordOrToken: "",
  expiresInDays: undefined as number | undefined,
});

const loading = ref(false);
const error = ref<string | null>(null);

const canSubmit = computed(() => {
  return form.value.host && form.value.username && form.value.passwordOrToken;
});

const submit = async () => {
  if (!canSubmit.value || loading.value) return;

  error.value = null;
  loading.value = true;

  try {
    if (props.editMode) {
      await credentialStore.update({
        host: form.value.host,
        username: form.value.username,
        newPassword: form.value.passwordOrToken,
        expiresInDays: form.value.expiresInDays,
      });
    } else {
      await credentialStore.add({
        host: form.value.host,
        username: form.value.username,
        passwordOrToken: form.value.passwordOrToken,
        expiresInDays: form.value.expiresInDays,
      });
    }

    emit("success");
    resetForm();
  } catch (e: any) {
    error.value = e.message || String(e);
    console.error("Failed to save credential:", e);
  } finally {
    loading.value = false;
  }
};

const cancel = () => {
  resetForm();
  emit("cancel");
};

const resetForm = () => {
  if (!props.editMode) {
    form.value = {
      host: "",
      username: "",
      passwordOrToken: "",
      expiresInDays: undefined,
    };
  }
  error.value = null;
};
</script>

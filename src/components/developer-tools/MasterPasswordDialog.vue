<template>
  <div v-if="show" class="modal modal-open">
    <div class="modal-box max-w-md">
      <h3 class="font-bold text-lg mb-4">
        {{ isFirstTime ? "设置主密码" : "解锁凭证存储" }}
      </h3>

      <!-- Error Alert -->
      <div v-if="error" class="alert alert-error alert-sm mb-4">
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
        <label class="label" for="master-password">
          <span class="label-text">主密码</span>
        </label>
        <input
          id="master-password"
          v-model="password"
          type="password"
          placeholder="请输入主密码"
          class="input input-bordered"
          @keyup.enter="submit"
          @input="error = null"
          autofocus
        />
      </div>

      <div v-if="isFirstTime" class="form-control mt-3">
        <label class="label" for="confirm-password">
          <span class="label-text">确认密码</span>
        </label>
        <input
          id="confirm-password"
          v-model="confirmPassword"
          type="password"
          placeholder="请再次输入密码"
          class="input input-bordered"
          @keyup.enter="submit"
          @input="error = null"
        />
      </div>

      <div v-if="isFirstTime && password" class="mt-4">
        <div class="flex justify-between text-xs mb-1">
          <span>密码强度</span>
          <span
            :class="{
              'text-error': strengthClass === 'weak',
              'text-warning': strengthClass === 'medium',
              'text-success': strengthClass === 'strong',
            }"
          >
            {{ strengthText }}
          </span>
        </div>
        <progress
          class="progress w-full"
          :class="{
            'progress-error': strengthClass === 'weak',
            'progress-warning': strengthClass === 'medium',
            'progress-success': strengthClass === 'strong',
          }"
          :value="strengthPercent"
          max="100"
        ></progress>
      </div>

      <div v-if="!isFirstTime" class="form-control mt-3">
        <label class="label cursor-pointer justify-start gap-2">
          <input
            v-model="rememberPassword"
            type="checkbox"
            class="checkbox checkbox-sm"
          />
          <span class="label-text">记住密码 (会话期间)</span>
        </label>
      </div>

      <div v-if="isFirstTime" class="alert alert-warning mt-4">
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
            d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
          />
        </svg>
        <span class="text-xs"
          >警告:
          请务必记住此密码！如果忘记密码，将无法恢复已存储的凭证数据。</span
        >
      </div>

      <div class="modal-action">
        <button class="btn btn-sm btn-ghost" @click="close" :disabled="loading">
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
          {{ loading ? "处理中..." : isFirstTime ? "设置" : "解锁" }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { useCredentialStore } from "../../stores/credential";

const props = defineProps<{
  show: boolean;
  isFirstTime?: boolean;
}>();

const emit = defineEmits<{
  close: [];
  success: [];
}>();

const credentialStore = useCredentialStore();

const password = ref("");
const confirmPassword = ref("");
const rememberPassword = ref(false);
const loading = ref(false);
const error = ref<string | null>(null);

// Password strength calculation
const passwordStrength = computed(() => {
  const pwd = password.value;
  if (!pwd) return 0;

  let strength = 0;
  // Length
  if (pwd.length >= 8) strength += 25;
  if (pwd.length >= 12) strength += 25;
  // Has lowercase
  if (/[a-z]/.test(pwd)) strength += 15;
  // Has uppercase
  if (/[A-Z]/.test(pwd)) strength += 15;
  // Has number
  if (/\d/.test(pwd)) strength += 10;
  // Has special char
  if (/[^a-zA-Z\d]/.test(pwd)) strength += 10;

  return Math.min(strength, 100);
});

const strengthPercent = computed(() => passwordStrength.value);

const strengthClass = computed(() => {
  const strength = passwordStrength.value;
  if (strength < 30) return "weak";
  if (strength < 60) return "medium";
  return "strong";
});

const strengthText = computed(() => {
  const strength = passwordStrength.value;
  if (strength < 30) return "弱";
  if (strength < 60) return "中等";
  return "强";
});

const canSubmit = computed(() => {
  if (!password.value) return false;
  if (props.isFirstTime) {
    return (
      password.value === confirmPassword.value && password.value.length >= 8
    );
  }
  return true;
});

const submit = async () => {
  if (!canSubmit.value || loading.value) return;

  error.value = null;
  loading.value = true;

  try {
    if (props.isFirstTime) {
      await credentialStore.setPassword(password.value);
    } else {
      await credentialStore.unlock(password.value);
    }

    emit("success");
    close();
  } catch (e: any) {
    error.value = e.message || String(e);
    console.error("Password operation failed:", e);
  } finally {
    loading.value = false;
  }
};

const close = () => {
  if (!loading.value) {
    password.value = "";
    confirmPassword.value = "";
    error.value = null;
    emit("close");
  }
};

// Clear fields when dialog opens/closes
watch(
  () => props.show,
  (newVal) => {
    if (newVal) {
      password.value = "";
      confirmPassword.value = "";
      error.value = null;
    }
  }
);
</script>

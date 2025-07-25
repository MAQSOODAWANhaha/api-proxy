<template>
  <router-view />
</template>

<script setup lang="ts">
import { watch } from 'vue'
import { useUserStore } from '@/stores/user'
import { useI18n } from 'vue-i18n'

const userStore = useUserStore()
const { locale } = useI18n()

// Watch for changes in the store's lang property and update i18n
watch(
  () => userStore.lang,
  (newLang) => {
    locale.value = newLang
  },
  { immediate: true } // Run immediately on component mount
)
</script>

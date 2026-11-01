<template>
  <div />
</template>

<script setup lang="ts">
//  入口。旧 UI の既定と同じスポーク画面を開く。
onMounted(() => {
  navigateTo("/spokes", { replace: true });
});
</script>

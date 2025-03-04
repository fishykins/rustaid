<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { writable } from "svelte/store";

  let imageSrc = writable("");

  async function fetchImage() {
    try {
      let base64Img = await invoke("get_frame");
      imageSrc.set(`data:image/png;base64,${base64Img}`);
    } catch (err) {
      console.error("Failed to fetch image:", err);
    }
  }

  onMount(() => {
    fetchImage();
    setInterval(fetchImage, 1000); // Poll every second
  });
</script>

<main>
  <h1>Live Window Capture</h1>
  {#if $imageSrc}
    <img src={$imageSrc} alt="Captured Frame" />
  {:else}
    <p>No image available</p>
  {/if}
</main>

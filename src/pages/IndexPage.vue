<template>
  <q-page class="row justify-center bg-grey-10 text-white q-pa-md">
    <div style="width: 100%; max-width: 500px">
      <div v-if="sessions.length === 0" class="text-center text-grey q-mt-xl">
        <q-spinner color="primary" size="2em" />
        <div class="text-caption">Searching Apps...</div>
      </div>

      <q-list bordered separator dense class="rounded-borders">
        <q-item v-for="session in sessions" :key="session.pid" class="q-py-ms items-center">
          
          <q-item-section avatar style="min-width: 40px; padding-right: 8px;">
            <q-avatar rounded size="32px">
              <img 
                v-if="session.icon_base64" 
                :src="`data:image/png;base64,${session.icon_base64}`" 
              />
              <q-icon v-else name="audiotrack" color="grey-6" />
            </q-avatar>
          </q-item-section>

          <q-item-section>
            <div class="row items-center justify-between no-wrap q-mb-xs">
              <div class="text-body2 text-weight-medium ellipsis" style="max-width: 180px;">
                {{ session.name }}
                <span class="text-caption text-grey-6" style="font-size: 0.7em;">#{{ session.pid }}</span>
              </div>
            </div>
            
            <div class="row items-center no-wrap">
              <q-slider
                class="col q-mr-sm"
                :model-value="session.volume"
                :min="0"
                :max="1"
                :step="0.01"
                dense
                color="primary"
                track-color="grey-8"
                thumb-size="14px" 
                @update:model-value="(val) => updateVolume(session.pid, val)"
                @pan-start="isDragging = true"
                @pan-end="isDragging = false"
              />
            </div>
            <div class="row q-gutter-xs justify-between q-pr-sm q-py-sm">
              <q-btn
                v-for="n in 11"
                :key="n"
                :label="(n - 1) * 10"
                size="xs"
                dense
                :color="[0, 5, 10].includes(n - 1) ? 'primary' : 'grey-9'"
                class="col" 
                @click="updateVolume(session.pid, (n - 1) / 10)"
              />
            </div>
          </q-item-section>

          <q-item-section side style="padding-left: 8px;">
            <div class="row items-center no-wrap q-gutter-x-xs">
              
              <q-input
                :model-value="Math.round(session.volume * 100)"
                type="number"
                filled 
                dark
                dense
                hide-bottom-space 
                class="rounded-borders"
                input-class="text-white"
                input-style="text-align: center; padding: 0;"
                style="width: 75px" 
                min="0"
                max="100"
                @update:model-value="(val) => updateFromInput(session.pid, val)"
                @focus="isDragging = true"
                @blur="isDragging = false"
              >
                <template v-slot:append>
                  <div class="column no-wrap justify-center">
                    <q-icon
                      name="arrow_drop_up"
                      size="xs"
                      class="cursor-pointer text-white hover-opacity"
                      style="height: 10px; line-height: 10px;"
                      @click.stop="updateFromInput(session.pid, Math.min(100, Math.round(session.volume * 100) + 1))"
                    />
                    <q-icon
                      name="arrow_drop_down"
                      size="xs"
                      class="cursor-pointer text-white hover-opacity"
                      style="height: 10px; line-height: 10px;"
                      @click.stop="updateFromInput(session.pid, Math.max(0, Math.round(session.volume * 100) - 1))"
                    />
                  </div>
                </template>
              </q-input>

              <q-btn
                dense
                flat
                round
                size="md"
                class="q-ml-sm"
                :color="session.muted ? 'red-5' : 'grey-4'"
                :icon="session.muted ? 'volume_off' : 'volume_up'"
                @click="toggleMute(session)"
              />
            </div>
          </q-item-section>
        </q-item>
      </q-list>
    </div>
  </q-page>
</template>

<script setup>
import { ref, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow, LogicalSize } from '@tauri-apps/api/window';

const sessions = ref([]);
let intervalId = null;
const isDragging = ref(false); 
const MAX_HEIGHT = 640;
let initialWidth = null;
let initialHeight = null;

async function fetchSessions() {
  if (isDragging.value) return;

  try {
    const data = await invoke('get_audio_sessions');  
    let requestedHeight = (data.length * 100) + 80;
    updateWindowSize(requestedHeight);
    sessions.value = data;
  } catch (e) {
    console.error('Fehler:', e);
  }
}

async function updateVolume(pid, volume) {
  const session = sessions.value.find(s => s.pid === pid);
  if (session) session.volume = volume;

  try {
    await invoke('set_app_volume', { pid, volume });
  } catch (e) { console.error(e); }
}

async function updateFromInput(pid, val) {
  if (val === null || val === '') return;
  let numVal = Number(val);
  if (numVal < 0) numVal = 0;
  if (numVal > 100) numVal = 100;
  await updateVolume(pid, numVal / 100.0);
}

async function toggleMute(session) {
  const newState = !session.muted;
  session.muted = newState;
  try {
    await invoke('set_app_mute', { pid: session.pid, muted: newState });
  } catch (e) {
    session.muted = !newState;
  }
}

onMounted(() => {
  fetchSessions();
  intervalId = setInterval(fetchSessions, 1000);
});

onUnmounted(() => {
  if (intervalId) clearInterval(intervalId);
});

async function updateWindowSize(requestedHeight) {
  const appWindow = getCurrentWindow();

    if (initialWidth === null) {
      const physicalSize = await appWindow.innerSize();
      const factor = await appWindow.scaleFactor();
      initialWidth = physicalSize.width / factor;
    }

    const newHeight = Math.max(initialHeight, requestedHeight);
    const targetHeight = Math.min(newHeight, MAX_HEIGHT);
    console.log(targetHeight)

    await appWindow.setSize(new LogicalSize(initialWidth, targetHeight));
}
</script>

<style scoped>
:deep(input[type=number]::-webkit-inner-spin-button), 
:deep(input[type=number]::-webkit-outer-spin-button) { 
  -webkit-appearance: none; 
  margin: 0; 
}

.hover-opacity:hover {
  opacity: 0.7;
}
</style>
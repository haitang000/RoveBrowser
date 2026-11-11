<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import Favicon from "../common/Favicon.vue";
import Icon from "../common/Icon.vue";
import { tabDisplayTitle } from "../../lib/tab-meta";
const b = useBrowser(),
  query = ref(""),
  input = ref<HTMLInputElement>();
const spaces = computed(() =>
  b.state.spaces.map((s) => ({
    ...s,
    tabs: b.state.tabs.filter(
      (t) =>
        t.spaceId === s.id &&
        `${tabDisplayTitle(t)} ${t.title} ${t.url}`.toLowerCase().includes(query.value.toLowerCase()),
    ),
  })),
);
/** A split half may be a live tab or the card's own tab; either way a rename wins. */
function titleFor(id: string, fallback: string) {
  const tab = b.state.tabs.find((x) => x.id === id);
  return tab ? tabDisplayTitle(tab) : fallback;
}
watch(
  () => b.deskOpen,
  async (open) => {
    if (open) {
      b.send({ type: "thumbnails" });
      query.value = "";
      await nextTick();
      input.value?.focus();
    }
  },
);
</script>
<template>
  <Transition name="desk"
    ><section
      v-if="b.deskOpen"
      class="desk"
      role="dialog"
      aria-modal="true"
      aria-label="全部标签页"
      @keydown.esc="b.deskOpen = false"
    >
      <div class="desk-tools">
        <div class="desk-search">
          <Icon name="search" /><input
            ref="input"
            v-model="query"
            placeholder="Search all tabs"
            aria-label="搜索全部标签页"
          />
        </div>
        <button
          class="icon-button"
          aria-label="关闭全部标签页视图"
          @click="b.deskOpen = false"
        >
          <Icon name="close" />
        </button>
      </div>
      <div class="desk-spaces">
        <section
          v-for="s in spaces"
          :key="s.id"
          class="desk-space"
          @dragover.prevent
          @drop.prevent="
            b.send({
              type: 'move',
              id: $event.dataTransfer?.getData('text/era-tab'),
              spaceId: s.id,
            });
            b.dragging = false;
          "
        >
          <h2>
            {{ s.name }} <small>{{ s.tabs.length }} tabs</small>
          </h2>
          <div class="desk-cards">
            <button
              v-for="t in s.tabs"
              :key="t.id"
              class="desk-card"
              :class="{ chosen: t.id === b.state.activeTab }"
              draggable="true"
              @dragstart="$event.dataTransfer?.setData('text/era-tab', t.id)"
              @mousedown.middle.prevent
              @auxclick.middle.prevent.stop="
                b.send({ type: 'close', id: t.id })
              "
              @click="
                b.send({ type: 'activate', id: t.id });
                b.deskOpen = false;
              "
            >
              <div
                class="desk-thumbnail"
                :class="{
                  'has-snapshot':
                    b.thumbnails[t.id] || b.thumbnails[t.splitWith || ''],
                  'split-snapshot': t.splitWith,
                }"
              >
                <template
                  v-for="id in [t.id, ...(t.splitWith ? [t.splitWith] : [])]"
                  :key="id"
                >
                  <img
                    v-if="b.thumbnails[id]"
                    class="tab-snapshot"
                    :src="b.thumbnails[id]"
                    :alt="`${titleFor(id, t.title)} 的真实网页缩略图`"
                    draggable="false"
                  />
                  <div v-else class="snapshot-empty">
                    <Favicon
                      :url="b.state.tabs.find((x) => x.id === id)?.url || t.url"
                    /><span>{{
                      titleFor(id, t.title)
                    }}</span
                    ><small>{{
                      t.url === "era://newtab"
                        ? "Rove"
                        : b.native
                          ? "尚无网页快照"
                          : "界面预览"
                    }}</small>
                  </div>
                </template>
              </div>
              <div class="desk-caption">
                <Favicon :url="t.url" /><span>{{ tabDisplayTitle(t) }}</span>
              </div>
            </button>
          </div>
          <p v-if="!s.tabs.length" class="empty-space">
            {{ query ? "没有匹配的标签页" : "拖动标签页到此工作区" }}
          </p>
        </section>
      </div>
    </section></Transition
  >
</template>

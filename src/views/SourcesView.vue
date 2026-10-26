<script setup lang="ts">
import { computed, nextTick, reactive, ref } from "vue";
import {
  NAlert,
  NButton,
  NForm,
  NFormItem,
  NInput,
  NModal,
  NTag,
  type FormInst,
  type FormRules,
  type InputInst,
} from "naive-ui";
import Icon from "../components/Icon.vue";
import {
  sourceLabels,
  sourceText,
  type Source,
  type SourceKind,
} from "../data/demo";
import { t } from "../i18n";

defineProps<{ sources: Source[]; selectedSourceId: string }>();
const emit = defineEmits<{
  select: [id: string];
  toggle: [source: Source];
  add: [source: Source];
}>();
const showDialog = ref(false);
const validationErrors = ref<string[]>([]);
const form = ref<FormInst>();
const nameInput = ref<InputInst>();
const urlInput = ref<InputInst>();
const draft = reactive({ name: "", kind: "maccms" as SourceKind, url: "" });
const sourceOptions = computed(() => [
  { value: "maccms", label: "MacCMS JSON" },
  { value: "stremio", label: t("Stremio add-on") },
]);
const rules = computed<FormRules>(() => ({
  name: {
    trigger: ["blur", "input"],
    validator: () =>
      draft.name.trim() ? true : new Error(t("Enter a name for this source.")),
  },
  url: {
    trigger: ["blur", "input"],
    validator: () => {
      let parsed: URL;
      try {
        parsed = new URL(draft.url);
      } catch {
        return new Error(
          t(
            "Enter a valid full URL, such as https://example.com/api.php/provide/vod/.",
          ),
        );
      }
      if (
        !["http:", "https:"].includes(parsed.protocol) ||
        parsed.username ||
        parsed.password ||
        parsed.hash
      )
        return new Error(
          t(
            "Enter a complete HTTP or HTTPS URL without credentials or a fragment.",
          ),
        );
      if (
        draft.kind === "stremio" &&
        !parsed.pathname.endsWith("/manifest.json")
      )
        return new Error(
          t("The Stremio add-on URL must end in /manifest.json."),
        );
      return true;
    },
  },
}));
function openDialog() {
  Object.assign(draft, { name: "", kind: "maccms", url: "" });
  validationErrors.value = [];
  showDialog.value = true;
}
async function saveSource() {
  if (!form.value) return;
  try {
    await form.value.validate();
  } catch (errors) {
    if (!Array.isArray(errors)) throw errors;
    validationErrors.value = errors
      .flat()
      .map((error) => String(error.message));
    await nextTick();
    (draft.name.trim() ? urlInput.value : nameInput.value)?.focus();
    return;
  }
  const url = new URL(draft.url);
  emit("add", {
    id: crypto.randomUUID(),
    name: draft.name.trim(),
    kind: draft.kind,
    url: url.href,
    enabled: true,
    demo: false,
    description: url.host,
  });
  showDialog.value = false;
}
</script>

<template>
  <div class="page-heading">
    <div>
      <p class="eyebrow">YOUR OWN SCREENING ROOM</p>
      <h1>{{ t("Good stories start here.") }}</h1>
      <p class="page-description">
        {{ t("Choose a source for your next screening.") }}
      </p>
    </div>
    <NButton type="primary" @click="openDialog"
      ><template #icon><Icon name="plus" :size="18" /></template
      >{{ t("Add source") }}</NButton
    >
  </div>
  <NAlert type="info" :bordered="false">{{
    t(
      "This preview uses demo catalogs. Added sources stay in this page session; live connections will follow.",
    )
  }}</NAlert>
  <div class="sources-list">
    <article
      v-for="item in sources"
      :key="item.id"
      :class="[
        'source-card',
        {
          'is-current': item.id === selectedSourceId,
          'is-disabled': !item.enabled,
        },
      ]"
    >
      <div class="source-emblem" :class="item.kind">
        <Icon
          :name="
            item.kind === 'cloudstream'
              ? 'sparkles'
              : item.kind === 'stremio'
                ? 'film'
                : 'layers'
          "
          :size="25"
        />
      </div>
      <div class="source-description">
        <div class="source-title">
          <h2>{{ sourceText(item).name }}</h2>
          <NTag size="small" :bordered="false">{{
            sourceLabels[item.kind]
          }}</NTag>
        </div>
        <p>{{ sourceText(item).description }}</p>
        <small v-if="item.url">{{ item.url }}</small
        ><span class="source-state"
          ><span :class="['status-dot', { dim: !item.enabled }]" />{{
            !item.enabled
              ? t("Disabled")
              : item.demo
                ? t("Demo source")
                : t("Added · Connection pending")
          }}</span
        >
      </div>
      <div class="source-actions">
        <NTag
          v-if="item.id === selectedSourceId"
          type="primary"
          :bordered="false"
          ><template #icon><Icon name="check" :size="16" /></template
          >{{ t("Current source") }}</NTag
        ><NButton
          v-else
          :disabled="!item.enabled"
          @click="emit('select', item.id)"
          >{{ t("Use source") }}</NButton
        ><NButton
          v-if="item.id !== selectedSourceId"
          quaternary
          @click="emit('toggle', item)"
          >{{ item.enabled ? t("Disable") : t("Enable") }}</NButton
        >
      </div>
    </article>
  </div>
  <div class="source-footnote">
    <Icon name="link" :size="17" />
    <p>
      {{
        t(
          "MacCMS JSON, Stremio add-ons and Cloudstream share one browsing and playback experience.",
        )
      }}
    </p>
  </div>
  <NModal
    v-model:show="showDialog"
    preset="card"
    role="dialog"
    :aria-label="t('Add a video source')"
    :title="t('Add a video source')"
    class="source-modal"
    transform-origin="center"
    :style="{ width: 'min(480px, calc(100vw - 32px))' }"
    @after-enter="nameInput?.focus()"
  >
    <p class="dialog-description">
      {{
        t(
          "Source configurations stay in this preview session so you can try the management flow.",
        )
      }}
    </p>
    <NAlert
      v-if="validationErrors.length"
      id="source-errors"
      type="error"
      role="alert"
      :show-icon="false"
      ><ul class="validation-summary">
        <li v-for="error in validationErrors" :key="error">{{ error }}</li>
      </ul></NAlert
    >
    <NForm
      ref="form"
      :model="draft"
      :rules="rules"
      @input="validationErrors = []"
      @submit.prevent="saveSource"
    >
      <NFormItem
        :label="t('Source name')"
        path="name"
        :label-props="{ for: 'source-name' }"
        ><NInput
          ref="nameInput"
          v-model:value="draft.name"
          :maxlength="40"
          :placeholder="t('For example: My film library')"
          :input-props="{
            id: 'source-name',
            autocomplete: 'off',
            'aria-describedby': validationErrors.length
              ? 'source-errors'
              : undefined,
          }"
      /></NFormItem>
      <NFormItem :label="t('Source type')" :label-props="{ for: 'source-kind' }"
        ><select
          id="source-kind"
          class="compact-select source-kind-select"
          v-model="draft.kind"
        >
          <option
            v-for="item in sourceOptions"
            :key="item.value"
            :value="item.value"
          >
            {{ item.label }}
          </option>
        </select></NFormItem
      >
      <NFormItem
        :label="t('API address')"
        path="url"
        :label-props="{ for: 'source-url' }"
        ><NInput
          ref="urlInput"
          v-model:value="draft.url"
          placeholder="https://example.com/api.php/provide/vod/"
          :input-props="{
            id: 'source-url',
            inputmode: 'url',
            autocomplete: 'url',
            'aria-describedby': validationErrors.length
              ? 'source-errors source-url-help'
              : 'source-url-help',
          }"
      /></NFormItem>
      <p id="source-url-help" class="field-help">
        {{ t("For Stremio, enter the full URL ending in manifest.json.") }}
      </p>
      <div class="dialog-actions">
        <NButton @click="showDialog = false">{{ t("Cancel") }}</NButton
        ><NButton attr-type="submit" type="primary">{{
          t("Add source")
        }}</NButton>
      </div>
    </NForm>
  </NModal>
</template>

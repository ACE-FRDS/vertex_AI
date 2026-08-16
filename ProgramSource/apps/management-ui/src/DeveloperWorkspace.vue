<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { X } from 'lucide-vue-next'
import { listWorkspaceDirectory, readWorkspaceFile } from './services/developer'

const emit = defineEmits(['close'])
const props = defineProps<{ workspaceId: string }>()
const mode = ref<'editor'|'preview'|'split'>('split')
const editorContent = ref(`// Welcome to Vertex Developer Workspace\n\nfunction hello() {\n  console.log('Hello Vertex AI Workspace')\n}\n`)
const entries = ref<string[]>([])

function close() {
  emit('close')
}

onMounted(async () => {
  if (!props.workspaceId) return
  try {
    const list = await listWorkspaceDirectory(props.workspaceId, '.')
    entries.value = list.split('\n')
  } catch (e) {
    entries.value = [`Error: ${String(e)}`]
  }
})

async function openEntry(line: string) {
  const parts = line.split('\t')
  const name = parts.pop()?.trim() ?? ''
  if (!name) return
  // If it's a file, read it
  if (!line.startsWith('directory')) {
    try {
      const content = await readWorkspaceFile(props.workspaceId, name)
      editorContent.value = content
      mode.value = 'editor'
    } catch (e) {
      editorContent.value = `Error: ${String(e)}`
      mode.value = 'editor'
    }
  }
}
</script>

<template>
  <section class="dev-workspace">
    <header class="dev-workspace-top">
      <div class="top-left">Vertex Developer Workspace</div>
      <div class="top-actions">
        <button class="button tertiary" @click="mode = 'editor'">EDITOR</button>
        <button class="button tertiary" @click="mode = 'preview'">PREVIEW</button>
        <button class="button tertiary" @click="mode = 'split'">SPLIT</button>
        <button class="icon-button" @click="close"><X :size="16" /></button>
      </div>
    </header>

    <div class="workspace-body">
      <aside class="explorer">
        <h4>Explorer</h4>
        <ul>
          <li v-for="(entry, idx) in entries" :key="idx"><button class="explorer-entry" @click="openEntry(entry)">{{ entry }}</button></li>
        </ul>
      </aside>

      <main class="main-panel">
        <div v-if="mode === 'editor' || mode === 'split'" class="editor-pane">
          <div class="editor-tabs">Editor - main.ts</div>
          <div class="editor-area">
            <pre class="line-numbers"><code v-for="(line, idx) in editorContent.split('\n')" :key="idx">{{ (idx+1)+'. ' + line }}</code></pre>
          </div>
        </div>

        <div v-if="mode === 'preview' || mode === 'split'" class="preview-pane">
          <div class="preview-tabs">Live Preview</div>
          <iframe src="about:blank" class="preview-frame"></iframe>
        </div>
      </main>

      <aside class="inspector">
        <h4>Inspector</h4>
        <div class="inspector-body">Agent / Context / Permissions</div>
      </aside>
    </div>

    <footer class="dev-terminal">
      <div class="terminal-left">
        <strong>Terminal</strong>
      </div>
      <div class="terminal-right">
        <div class="terminal-output">$ echo \"Preview not connected\"\nPreview not connected</div>
        <input placeholder="Type a command (mock)" />
      </div>
    </footer>
  </section>
</template>

<style scoped>
.dev-workspace { border: 1px solid var(--line); border-radius: 8px; padding: 0; background: linear-gradient(180deg, rgba(17,25,35,.96), rgba(10,18,26,.98)); margin-bottom: 12px; }
.dev-workspace-top { display:flex; align-items:center; justify-content:space-between; padding: 12px 16px; border-bottom: 1px solid var(--line); }
.top-actions { display:flex; gap:8px; align-items:center }
.workspace-body { display:flex; gap:12px; padding:12px; }
.explorer { width:220px; border-right:1px solid var(--line); padding-right:12px }
.main-panel { flex:1; display:flex; gap:12px }
.editor-pane { flex:1; display:flex; flex-direction:column }
.editor-tabs { padding:8px 10px; border-bottom:1px solid var(--line); color:var(--muted) }
.editor-area { padding:12px; background: #041018; border-radius:6px; min-height:240px; overflow:auto }
.line-numbers { font-family: 'DM Mono', monospace; color: #c9d2db; margin:0 }
.preview-pane { width:420px; display:flex; flex-direction:column }
.preview-frame { flex:1; border:1px solid var(--line); border-radius:6px; background:white }
.inspector { width:260px; border-left:1px solid var(--line); padding-left:12px }
.dev-terminal { display:flex; gap:12px; padding:10px 12px; border-top:1px solid var(--line); align-items:flex-start }
.terminal-output { background: #02060a; color:#9aa8b9; padding:10px; border-radius:6px; min-width:320px; min-height:48px }
.icon-button { background:transparent; border:0; color:var(--muted) }
.button.tertiary { background:transparent; border:1px solid var(--line); color:var(--muted); padding:6px 9px; border-radius:6px }
</style>

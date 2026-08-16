<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, watch, computed } from 'vue'
import { X } from 'lucide-vue-next'
import * as monaco from 'monaco-editor'
import { listWorkspaceDirectory, readWorkspaceFile, writeWorkspaceFile, listDeveloperWorkspaces } from './services/developer'

const emit = defineEmits(['close'])
const props = defineProps<{ workspaceId: string }>()
const mode = ref<'editor'|'preview'|'split'>('split')
const editorContent = ref(`// Welcome to Vertex Developer Workspace\n\nfunction hello() {\n  console.log('Hello Vertex AI Workspace')\n}\n`)
const entries = ref<any[]>([])
// store child entries for directories
let childrenMap: Record<string, Array<any>> = {}
let expandedDirs: Record<string, boolean> = {}


const visibleNodes = computed(() => {
  const result: Array<{entry:any, depth:number}> = []
  function walk(list: Array<any>, depth: number) {
    for (const entry of list) {
      result.push({ entry, depth })
      if (entry.kind === 'directory' && expandedDirs[entry.relative]) {
        const children = childrenMap[entry.relative] || []
        walk(children, depth + 1)
      }
    }
  }
  try { walk(entries.value, 0) } catch (e) {}
  return result
})
const currentFileName = ref<string | null>(null)
const dirty = ref(false)
const saving = ref(false)
const message = ref<string | null>(null)

let editorInstance: monaco.editor.IStandaloneCodeEditor | null = null
let modelMap: Record<string, monaco.editor.ITextModel> = {}
const editorContainer = ref<HTMLElement | null>(null)
// meta per-file for SFCs
let metaMap: Record<string, { scriptLang?: string }> = {}
const currentSubModel = ref<'template'|'script'|'style'|null>(null)
const activeVueFile = ref<string | null>(null)

// workspace info for display
const workspaceInfo = ref<{ id: string; name: string; root: string } | null>(null)

function close() {
  emit('close')
}

onMounted(async () => {
  // Load workspace info for header display
  try {
    const workspaces = await listDeveloperWorkspaces()
    workspaceInfo.value = workspaces.find((w:any) => w.id === props.workspaceId) ?? null
  } catch (e) {
    workspaceInfo.value = null
  }

  if (props.workspaceId) {
    try {
      const list = await listWorkspaceDirectory(props.workspaceId, '.')
        // list is WorkspaceEntry[]
        entries.value = list
        // Clear cached children
        childrenMap = {}
        expandedDirs = {}
      } catch (e) {
        entries.value = [`Error: ${String(e)}`]
      }
    } else {
      entries.value = ['Workspace Not Selected']
    }

  if (editorContainer.value) {
    editorInstance = monaco.editor.create(editorContainer.value, {
      value: editorContent.value,
      language: determineLanguage(currentFileName.value),
      automaticLayout: true,
      theme: 'vs-dark',
      minimap: { enabled: false }
    })

    editorInstance.onDidChangeModelContent(() => {
      const val = editorInstance?.getValue() ?? ''
      editorContent.value = val
      dirty.value = true
      message.value = null
    })
  }
})

onBeforeUnmount(() => {
  if (editorInstance) {
    editorInstance.dispose()
    editorInstance = null
  }
  // Dispose models
  for (const k of Object.keys(modelMap)) {
    modelMap[k].dispose()
  }
  modelMap = {}
})

async function fetchChildren(relative: string) {
  if (childrenMap[relative]) return
  try {
    const list = await listWorkspaceDirectory(props.workspaceId, relative)
    childrenMap[relative] = list
  } catch (e) {
    childrenMap[relative] = []
  }
}

function toggleDir(entry:any) {
  const obj = typeof entry === 'string' ? JSON.parse(entry) : entry
  const rel = obj.relative
  expandedDirs[rel] = !expandedDirs[rel]
  if (expandedDirs[rel]) {
    void fetchChildren(rel)
  }
}

watch(() => editorContent.value, (v) => {
  if (editorInstance && editorInstance.getValue() !== v) {
    const sel = editorInstance.getSelection()
    editorInstance.setValue(v)
    if (sel) editorInstance.setSelection(sel)
  }
})

function determineLanguage(fileName: string | null) {
  if (!fileName) return 'typescript'
  if (fileName.endsWith('.ts') || fileName.endsWith('.tsx')) return 'typescript'
  if (fileName.endsWith('.js') || fileName.endsWith('.jsx')) return 'javascript'
  if (fileName.endsWith('.rs')) return 'rust'
  if (fileName.endsWith('.vue')) return 'html'
  if (fileName.endsWith('.css')) return 'css'
  return 'plaintext'
}

async function openEntry(line: string) {
  const parts = line.split('\t')
  const name = parts.pop()?.trim() ?? ''
  if (!name) return
  // If it's a file, read it
  if (!line.startsWith('directory')) {
    try {
      const content = await readWorkspaceFile(props.workspaceId, name)
      editorContent.value = content
      currentFileName.value = name
      dirty.value = false
      mode.value = 'editor'
      // update monaco model
      if (editorInstance) {
        const lang = determineLanguage(name)
        if (name.endsWith('.vue')) {
          const parts = parseVueSFC(content)
          const tplKey = `${name}::template`
          const scriptKey = `${name}::script`
          const styleKey = `${name}::style`

          let tplModel = modelMap[tplKey]
          if (!tplModel) { tplModel = monaco.editor.createModel(parts.template || '', 'html'); modelMap[tplKey] = tplModel } else { tplModel.setValue(parts.template || '') }

          const scriptLang = parts.scriptLang === 'ts' ? 'typescript' : 'javascript'
          let scriptModel = modelMap[scriptKey]
          if (!scriptModel) { scriptModel = monaco.editor.createModel(parts.script || '', scriptLang); modelMap[scriptKey] = scriptModel } else { scriptModel.setValue(parts.script || '') }

          let styleModel = modelMap[styleKey]
          if (!styleModel) { styleModel = monaco.editor.createModel(parts.style || '', 'css'); modelMap[styleKey] = styleModel } else { styleModel.setValue(parts.style || '') }

          metaMap[name] = { scriptLang: parts.scriptLang }
          activeVueFile.value = name
          currentSubModel.value = 'template'
          editorInstance.setModel(tplModel)
          // sync editorContent
          editorContent.value = tplModel.getValue()
        } else {
          let model = modelMap[name]
          if (!model) {
            model = monaco.editor.createModel(content, lang)
            modelMap[name] = model
          } else {
            model.setValue(content)
          }
          editorInstance.setModel(model)
        }
      }
    } catch (e) {
      editorContent.value = `Error: ${String(e)}`
      currentFileName.value = null
      mode.value = 'editor'
    }
  }
}

function onEditorInput() { dirty.value = true; message.value = null }

function parseVueSFC(content: string) {
  const templateMatch = content.match(/<template[^>]*>([\s\S]*?)<\/template>/i)
  const scriptMatch = content.match(/<script(?:\s+lang=["']?(ts|js)["']?)?[^>]*>([\s\S]*?)<\/script>/i)
  const styleMatch = content.match(/<style[^>]*>([\s\S]*?)<\/style>/i)
  return {
    template: templateMatch ? templateMatch[1].trim() : '',
    script: scriptMatch ? scriptMatch[2].trim() : '',
    scriptLang: scriptMatch ? (scriptMatch[1] ? scriptMatch[1] : 'js') : 'js',
    style: styleMatch ? styleMatch[1].trim() : ''
  }
}

function buildVueSFC(name: string) {
  const tpl = modelMap[`${name}::template`]?.getValue() ?? ''
  const script = modelMap[`${name}::script`]?.getValue() ?? ''
  const style = modelMap[`${name}::style`]?.getValue() ?? ''
  const scriptModel = modelMap[`${name}::script`]
  const scriptLangAttr = (scriptModel && (scriptModel.getLanguageId ? scriptModel.getLanguageId().includes('typescript') : false)) ? ' lang="ts"' : ''
  let res = ''
  res += `<template>\n${tpl}\n` + '</' + `template>\n\n`
  res += `<script${scriptLangAttr}>\n${script}\n` + '</' + `script>\n\n`
  if (style.trim()) res += `<style>\n${style}\n` + '</' + `style>\n`
  return res
}

async function saveFile() {
  // Validate preconditions and show clear errors instead of silent returns
  if (!props.workspaceId) {
    message.value = 'Workspace Not Selected'
    return
  }
  if (!currentFileName.value) {
    message.value = 'No File Open'
    return
  }

  // Ensure file is under workspace root if known
  if (workspaceInfo.value && !currentFileName.value.startsWith(workspaceInfo.value.root) && !currentFileName.value.startsWith('.')) {
    // If the filename is an absolute path outside workspace, reject
    message.value = 'File is outside the selected Workspace'
    return
  }

  saving.value = true
  message.value = null
  try {
    let contentToWrite = editorContent.value
    if (currentFileName.value.endsWith('.vue') && activeVueFile.value === currentFileName.value) {
      contentToWrite = buildVueSFC(currentFileName.value)
    }
    const ok = await writeWorkspaceFile(props.workspaceId, currentFileName.value, contentToWrite)
    if (ok) {
      dirty.value = false
      message.value = 'Saved'
    } else {
      message.value = 'Save failed'
    }
  } catch (e) {
    message.value = `Error: ${String(e)}`
  } finally {
    saving.value = false
  }
}

function switchVueSubModel(kind: 'template'|'script'|'style') {
  if (!activeVueFile.value || !editorInstance) return
  const name = activeVueFile.value
  const key = `${name}::${kind}`
  const model = modelMap[key]
  if (model) {
    editorInstance.setModel(model)
    currentSubModel.value = kind
    editorContent.value = model.getValue()
  }
}
</script>

<template>
  <section class="dev-workspace">
    <header class="dev-workspace-top">
        <div class="top-left">
          Vertex Developer Workspace
          <div class="workspace-meta">
            <span v-if="workspaceInfo">{{ workspaceInfo.name }} — <small>{{ workspaceInfo.root }}</small></span>
            <span v-else class="muted">Workspace Not Selected</span>
          </div>
      </div>
        <div class="top-actions">
          <span class="current-file" v-if="currentFileName">File: {{ currentFileName }}</span>
          <button class="button primary" @click="saveFile" :disabled="saving">Save</button>
          <button class="button tertiary" @click="mode = 'editor'">EDITOR</button>
          <button class="button tertiary" @click="mode = 'preview'">PREVIEW</button>
          <button class="button tertiary" @click="mode = 'split'">SPLIT</button>
          <button class="icon-button" @click="close"><X :size="16" /></button>
        </div>
        <div class="top-status"><span v-if="message" :class="{ muted: message === 'Workspace Not Selected' || message === 'No File Open' || message === 'File is outside the selected Workspace' || message.startsWith('Error') }">{{ message }}</span></div>
      </header>

    <div class="workspace-body">
      <aside class="explorer">
        <h4>Explorer</h4>
        <div class="explorer-list">
          <div v-for="node in visibleNodes" :key="node.entry.relative" :style="{ paddingLeft: `${node.depth * 12}px` }" class="explorer-row" :class="{ directory: node.entry.kind === 'directory', file: node.entry.kind === 'file', selected: currentFileName === node.entry.relative }">
            <template v-if="node.entry.kind === 'directory'">
              <button class="explorer-toggle" @click.prevent="toggleDir(node.entry)">{{ expandedDirs[node.entry.relative] ? '▾' : '▸' }}</button>
              <span class="icon">📁</span>
              <button class="explorer-name" @click.prevent="toggleDir(node.entry)">{{ node.entry.name }}</button>
            </template>
            <template v-else>
              <span class="spacer"></span>
              <span class="icon">📄</span>
              <button class="explorer-name" @click.prevent="openEntry(node.entry)">{{ node.entry.name }}</button>
            </template>
          </div>
        </div>
      </aside>

      <main class="main-panel">
        <div v-if="mode === 'editor' || mode === 'split'" class="editor-pane">
          <div class="editor-tabs">
            <span>{{ currentFileName ?? 'untitled' }}</span>
            <div style="margin-left:auto; display:flex; gap:8px; align-items:center">
              <template v-if="currentFileName && currentFileName.endsWith('.vue')">
                <button class="button tertiary" :class="{ active: currentSubModel === 'template' }" @click.prevent="switchVueSubModel('template')">Template</button>
                <button class="button tertiary" :class="{ active: currentSubModel === 'script' }" @click.prevent="switchVueSubModel('script')">Script</button>
                <button class="button tertiary" :class="{ active: currentSubModel === 'style' }" @click.prevent="switchVueSubModel('style')">Style</button>
              </template>
              <button class="button tertiary" @click="saveFile" :disabled="!currentFileName || !dirty || saving">{{ saving ? 'Saving...' : 'Save' }}</button>
              <span v-if="message" class="muted">{{ message }}</span>
            </div>
          </div>
          <div class="editor-area">
            <div ref="editorContainer" class="editor-container" style="height:100%; min-height:240px"></div>
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
.explorer { width:260px; border-right:1px solid var(--line); padding-right:12px; overflow:auto; max-height:520px }
.main-panel { flex:1; display:flex; gap:12px }
.editor-pane { flex:1; display:flex; flex-direction:column }
.editor-tabs { padding:8px 10px; border-bottom:1px solid var(--line); color:var(--muted); display:flex; align-items:center; gap:12px }
.editor-area { padding:12px; background: #041018; border-radius:6px; min-height:240px; overflow:auto }
.editor-textarea { width:100%; height:100%; min-height:240px; background:#041018; color:#c9d2db; border:0; outline:none; padding:12px; font-family: 'DM Mono', monospace; resize:vertical }
.line-numbers { font-family: 'DM Mono', monospace; color: #c9d2db; margin:0 }
.preview-pane { width:420px; display:flex; flex-direction:column }
.preview-frame { flex:1; border:1px solid var(--line); border-radius:6px; background:white }
.explorer-list { display:flex; flex-direction:column }
.explorer-row { display:flex; align-items:center; gap:8px; padding:6px 4px; white-space:nowrap; overflow:hidden }
.explorer-row .icon { width:18px }
.explorer-row .explorer-name { background:transparent; border:0; color:var(--muted); text-align:left; padding:0; white-space:nowrap; overflow:hidden; text-overflow:ellipsis }
.explorer-row.selected { background: rgba(255,255,255,0.03); border-radius:4px }
.explorer-toggle { background:transparent; border:0; width:18px }
.explorer { font-size:13px }

.inspector { width:260px; border-left:1px solid var(--line); padding-left:12px }
.dev-terminal { display:flex; gap:12px; padding:10px 12px; border-top:1px solid var(--line); align-items:flex-start }
.terminal-output { background: #02060a; color:#9aa8b9; padding:10px; border-radius:6px; min-width:320px; min-height:48px }
.icon-button { background:transparent; border:0; color:var(--muted) }
.button.tertiary { background:transparent; border:1px solid var(--line); color:var(--muted); padding:6px 9px; border-radius:6px }
.muted { color: var(--muted); font-size: 12px }
</style>

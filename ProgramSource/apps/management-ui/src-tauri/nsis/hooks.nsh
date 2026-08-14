; Vertex AI keeps Memory data by default. The runtime itself is removed with
; the application; the durable cluster remains under the per-user data root.
!macro NSIS_HOOK_PREUNINSTALL
  MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON1 "Vertex AIをアンインストールします。AI Memoryを保持しますか？$\r$\n$\r$\n「はい」: アプリのみ削除し、AI Memoryを保持します。$\r$\n「いいえ」: AI Memoryも完全に削除します。" IDYES vertex_memory_keep
  MessageBox MB_YESNO|MB_ICONEXCLAMATION|MB_DEFBUTTON2 "AI Memoryを完全に削除します。この操作は元に戻せません。続行しますか？" IDNO vertex_memory_keep
  RMDir /r "$APPDATA\com.vertexproject.ai\Memory"
  vertex_memory_keep:
!macroend

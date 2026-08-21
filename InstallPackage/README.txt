Vertex AI 0.1.6 テスト配布パッケージ
====================================

対象OS: Windows 10 22H2以降 / Windows 11（x64）
最新インストーラー: Vertex-AI-0.1.6-x64-setup.exe

0.1.6の主な追加
---------------
- ARD Role / Capability / Registry / Runtime / Hardwareに基づくBrain: Auto解決
- Architect / Developer / Reviewer / Custom RoleのTyped Capability Profile
- 明示Brain優先、Auto限定の有界Fallback、安定Tie-Break
- Runtime非依存のTyped Model Runtime Adapter
- Ollama実ロード状態の観測、Preload、Unload、同一モデル再利用
- Single GPU向けHandoff後のUnload / Load Rotation
- Rotation再試行、Pause / Cancel、異常終了時のInterrupted復旧
- Brain Resolution理由、Compatibility、Score、Runtime状態のUI表示
- qwen3:8bを用いた実Ollama Auto ARD Self Development受入テスト

0.1.5の主な追加
---------------
- Runtime非依存のTyped Model Registry
- 任意フォルダを複数登録できるModel Storage Manager
- Native Folder Picker、Default Storage変更、切断Storage保持
- GGUFの軽量DiscoveryとDuplicate候補検出
- Ollama Discovery Adapterによる共通Registry統合
- Coding / Reasoning / Review等のTyped Capability
- RAM、VRAM使用量、Storage空き容量を用いたCompatibility判定
- ARD Brain: Auto向けCapability / Hardware / Runtime候補解決境界
- 日本語中心のModel Manager UI

0.1.4の主な追加
---------------
- ARD Execution BridgeによるArchitect / Developer / Reviewer自動Relay
- 型付きAgentExecutionRequest / AgentExecutionResult / ReviewResult
- 実ファイル変更、cargo check、cargo testの決定論的完了条件
- Ollama Structured OutputとQwen3思考モード制御
- Pause / Resume / CancelのDeveloper Task伝播
- ARD実行履歴、Activity、再起動時Interrupted復旧
- UIからのSession自動ポーリングと進捗反映

0.1.3の主な追加
---------------
- PostgreSQL 18.4をVertex Memory Core専用Runtimeとして内蔵
- 初回起動時の専用Cluster、Role、vertex_ai Database自動生成
- Windows資格情報マネージャーによるDB Credential保護
- localhost限定・動的専用Port・競合時の再選択
- Memory Schema Migration、自動起動・停止・再起動・診断
- PostgreSQL停止時もアプリとローカルAIを継続する縮退動作
- AI Memoryの実データ検索・保存とRuntime技術詳細UI
- アンインストール時はAI Memory保持を既定選択

前提条件
--------
- AI MemoryにPostgreSQLの別途インストールは不要です。
- ローカルAIを使用する場合のみOllamaを起動してください。
- 推奨モデルは qwen3:8b です。
- GGUF保存先はモデル画面の「保存先を追加」から選択できます。
- ブラウザプレビューではOS情報とOllamaへ直接アクセスしません。

注意事項
--------
- 現在は開発途中のテスト配布版です。
- コード署名証明書による署名はまだ行っていません。
  Windows SmartScreenの警告が表示される場合があります。
- Ollamaとの通信はlocalhost（127.0.0.1:11434）に限定しています。
- AI生成に失敗してもクラウドへ自動転送しません。

整合性確認
----------
SHA256SUMS.txtにSHA-256ハッシュを記載しています。

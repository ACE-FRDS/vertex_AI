# Vertex AI 開発状況

最終更新: 2026-08-14

## 現在地

完全版マスター仕様を正本として、Vertex Environment Explorer、安全な端末管理、Local Runtime Managementを実装中。現在はOllamaモデルをUIからバックグラウンド取得できる段階まで到達した。

## 実装済み

- Rustワークスペースとトランスポート非依存Coreコマンド
- プロバイダー共通抽象化とOpenAI Responses APIアダプター
- 手動モデル選択とモデルレジストリ
- OS秘密情報ストア抽象化と平文フォールバック禁止
- PostgreSQL向け構造化Memoryスキーマ、リポジトリ、スコープ境界
- Vertex Context構築とクラウド向けプライバシーフィルター
- Vue 3 / TypeScript管理UI
- 日本語標準表示、日本語・英語切替、端末内設定保存
- Vertex FM ENGINE準拠の配色と可読性調整
- SystemAsset、EnvironmentFinding、HealthCheck、ErrorEnvelope、RepairPlan、AuditEventの共有契約
- 読み取り専用PATHスキャンによる開発・制作・AIツール検出の最小実装
- 能力メタデータと`provides`関係の生成
- Tauri 2デスクトップシェルと最小権限capability
- Tauri IPCの`scan_environment`コマンド
- 実スキャン結果を表示する環境エクスプローラー画面
- ブラウザプレビューとデスクトップ端末アクセスの安全な分離
- Windows用Vertex AIアプリアイコン
- Environment Indexのスキーマバージョン付き永続キャッシュ
- 追加・更新・削除を区別する環境差分検出
- 原子的保存、旧版バックアップ、置換失敗時の復元
- 起動時の前回スナップショット読込と再スキャン
- Ollamaローカルプロバイダー（モデル検出、ヘルスチェック、生成）
- ローカルAI通信のループバック限定とクラウド自動フォールバック禁止
- ローカル限定Vertex Contextと生成入力上限
- 60秒間隔の環境・ローカルモデルバックグラウンド更新
- 5 MiBローテーション付き永続監査ログ
- 実Ollamaモデルを使用する最小AIテストコンソール
- Windows x64向けNSISインストールパッケージ生成
- バックグラウンドモデル取得機能を収録したVertex AI 0.1.2 Windows x64 NSISインストーラー
- プロバイダー非依存のLocal Runtime Manager契約とCoreレジストリ
- Ollamaのバージョン、実行ファイル、モデル保存先、導入モデル、ロード状態、VRAM使用量の検出
- AI Environment Managerによる複数ランタイム・モデル資産の集約
- 監査ログ付きのロード済みモデルの安全なメモリ解放
- 「AI環境マネージャー」「ローカルランタイム」管理画面
- Ollama `/api/pull`ストリームを使用したモデルのバックグラウンドダウンロード
- ダウンロード進捗、重複実行防止、キャンセル、失敗理由、完了履歴
- アプリ再起動後も履歴を保持し、実行中だったジョブを「中断」として復元する永続ジョブ台帳
- モデル保存先ドライブの総容量、使用率、空き容量の検出と表示
- モデル管理画面の日本語ダウンロードUI（英語切替対応）
- モデルダウンロード開始・キャンセルの構造化監査記録
- Rustテスト43件成功、外部環境依存テスト2件は明示的に除外、Clippy警告ゼロ、UI型検査・本番ビルド成功
- DashboardをOperational Cockpit / Action Centerへ再設計
- ARD（Agent Relay Development）MVP Core
- 任意人数のAI Team Member、RoleとModel（Brain: Auto / 明示Model）の分離
- 担当者別Soft Role PolicyとTool LayerのHard Permission
- Typed Structured Handoff、ReviewerからDeveloperへの差し戻し、Retry上限
- ARD SessionのPause / Resume / Cancel、ユーザー介入、再起動後のInterrupted Recovery
- 単一GPUを前提としたModel Rotation記録と同一Model再利用判定
- ARD Team / Workflow / Sessionの原子的JSON永続化とTauri IPC
- Developer画面の「AIチーム・人事」、Workflow、ARD Activity最小UI
- Ollama、導入モデル、ロード状態、VRAM割当、モデル保存先、空き容量、モデル取得ジョブを実データで集約表示
- 未接続のCPU・RAM・GPU・PostgreSQL・Knowledge Core・AI Agent状態を安全な「API接続待ち」として表示
- 実環境から導出する対応項目とVertex推奨の詳細画面導線
- AIアクティビティ、Active Models、Resource Monitor、System Healthの将来接続可能なUI構造
- 履歴グラフを補助領域へ縮小し、プレビューデータであることを明示

## 一部実装

- Phase 0: Tauriシェルと最初のUI-Core IPCは実装済み。実監査ログ永続化と全管理コマンドの接続は未実装
- Phase 1: OpenAIアダプターとOllamaローカル生成を実装済み。OpenAIのデスクトップIPC、ストリーミング、キャンセルは未実装
- Phase 2: PATH上の代表ツール検出、永続インデックス、差分検出、キャッシュ検索、定期バックグラウンド更新、モデル保存先の容量監視を実装済み。イベント駆動ウォッチャー、広範なアプリ検出、重複を含む完全なストレージ棚卸しは未実装
- ARD MVP: Team/Role/Permission/Handoff/Workflow/状態永続化/UI境界は実装済み。ARD Stageから既存Developer Agentを自動起動する実行Bridge、Runtimeの実Load/Unload、Project Brain/PostgreSQL連携は未実装

## 未実装の主要領域

- モデル重複検出、互換性分類、安全な削除・移行
- PATH、サービス、タスク、レジストリのSystem Health診断
- Error Intelligenceと診断コンテキスト収集
- Creator Capability Graphの本格実装
- 承認、バックアップ、検証、ロールバックを伴うSmart Fix
- Knowledge CoreとADR取り込み
- Auto／Councilルーティング
- Built-in Local Inference Runtime（OllamaはExternal Runtimeとして継続）
- ARD Auto BrainのModel Router実解決、Hardware/VRAM適合判定
- Model Registryの任意Storage、移動、消失・重複検出
- ARD HandoffとDecisionのProject Brain / Knowledge Core保存

## 既知の環境上の制約

- この端末ではWindows Credential Managerの実書き込みがプラットフォームエラーになる場合がある。アダプターは安全側に失敗し、平文保存しない。
- ローカルのPostgreSQL実接続テストは接続先が設定されていないため既定で除外する。インメモリの境界テストは実行する。
- この端末ではOllama 0.32.9と`qwen3:8b`を確認済み。Ollama停止時は安全に利用不可と判定し、クラウドへ自動転送しない。

## 次の実装順序

1. ARD Stage → Developer Agent Task実行BridgeとStructured Handoff自動生成
2. Ollama RuntimeでのRelay実行、担当交代時の安全なUnload/Load
3. ARD State / Handoff / DecisionのPostgreSQL Project Brain統合
4. Installed Model Registryと任意Storage Location
5. Built-in Local Runtime技術選定と配布PoC

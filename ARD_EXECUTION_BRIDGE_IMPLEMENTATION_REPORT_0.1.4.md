# Vertex AI ARD Execution Bridge 実装報告 0.1.4

## 結果

ARD MVP Control Planeと既存Vertex Developer Agentを実働接続し、Architect、Developer、Reviewerを自動RelayするExecution Bridgeを実装した。

実Ollama `qwen3:8b` を使用した最小Repository受入試験で、解析、実ファイル変更、`cargo check`、`cargo test`、構造化Review承認、ARD Session完了まで成功した。

## 実装範囲

- 型付き `AgentExecutionRequest` / `AgentExecutionResult` / `ReviewResult`
- Explicit Brain解決とAuto時のローカルOllamaフォールバック
- ARD Stageから既存Developer Agent Taskへの実行接続
- Role PolicyとHard PermissionのDeveloper Tool境界への伝播
- 実Commandからのみ生成するBuild/Test Fact
- 実DiffとCommand Factを使用するReviewer判定
- Review拒否時のRework Relayと既存Retry上限
- Pause / Resume / Cancelの実行中Developer Taskへの伝播
- User Interventionの次回安全Contextへの注入
- Active Execution履歴と再起動時Interrupted / Paused復旧
- UI Session PollingとActivity更新
- Ollama `think: false` とAgentAction JSON Schema Structured Output

## 安全性

- LLMのBuild/Test自己申告は完了根拠に使用しない。
- ARD Developerは実ファイル変更、実`cargo check`、実`cargo test`が揃うまで完了できない。
- 実行ファイルと引数を分離し、Build/TestフェーズはSchemaで安全なCargo Commandへ拘束する。
- Workspace Sandbox、Secret Protection、Terminal Allowlist、Risk/Permission判定は既存Developer Agentを再利用する。
- Activityには操作、状態、結果を記録し、モデルの思考過程は保存しない。

## 検証結果

- `cargo test --workspace`: 成功
- `cargo clippy --workspace --all-targets -- -D warnings`: 成功
- `pnpm build`: 成功
- 実Ollama READ ONLY受入試験: 成功
- 実Ollama ARD Self Development Loop受入試験: 成功（33.42秒）
- Tauri Release / NSIS Build: 成功

## インストーラー

- Version: 0.1.4
- File: `InstallPackage/Vertex-AI-0.1.4-x64-setup.exe`
- Size: 35,386,221 bytes
- SHA-256: `27ef4984c05ffde7fc6f8c51c88809828b608fd6dca465edf1e3554618a3213b`
- Code Signing: 未署名

## 既知の制限

- Auto Brainは正式なVertex Orchestrator接続前のため、利用可能なローカルOllamaモデルを安全な暫定Resolverで選択する。
- コード署名証明書は未導入のため、Windows SmartScreen警告が表示される場合がある。
- PostgreSQL実Runtime統合試験は専用環境変数を必要とするため、通常のWorkspace TestではIgnore対象となる。

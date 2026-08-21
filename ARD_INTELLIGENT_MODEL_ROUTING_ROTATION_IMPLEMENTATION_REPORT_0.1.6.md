# Vertex AI 0.1.6 実装報告

## Version

Vertex AI 0.1.6 — ARD Intelligent Model Routing & Rotation

## Architecture

ARD Coreと個別Runtime APIを分離し、`ArdEngineResolver`、`ArdRuntimeController`、`ModelRuntimeAdapter`の3境界で構成した。ARDはOllama APIを直接呼ばず、Model RegistryとRuntime Adapterを介してBrain解決と実ロード制御を行う。

## Implemented Features

- Role → Capability → Model Registry → Runtime → Hardware → PolicyのTyped Brain Resolution
- Architect、Developer、Reviewer、Custom RoleのCapability Profile
- Brain: Autoの決定論的候補順位、安定Tie-Break、候補除外
- 明示Brainの優先と、明示指定時の自動Fallback禁止
- `Balanced`を既定とするTyped Resource Policy境界
- Hardware Compatibilityの実行前確認
- Loaded / Loading / Unloading / Unloaded / Unavailable / ErrorのRuntime状態
- observedフラグによる実測状態と推定状態の分離
- 現在モデル、次モデル、Runtime、再利用可否を持つModel Rotation Plan
- 同一モデルの実ロード確認と再利用
- Handoff永続化後のUnload → LoadによるSingle GPU向けRotation
- Rotation失敗時の最大2回再試行と、Auto時のみ1候補の代替解決
- Pause / CancelのRuntime操作への伝播
- Rotation中の異常終了をInterruptedとして永続復旧
- Brain Resolution、Rotation Event、ActivityのSession永続化

## Ollama Runtime Adapter

- `/api/ps`で実際にロード済みのモデルを観測
- 空Promptと`keep_alive: -1`でモデルをPreloadして保持
- 空Promptと`keep_alive: 0`でモデルをUnload
- 各操作後に実状態を再観測し、Loaded / Unloaded到達を確認

## UI

- Auto選定の標準3担当ARDチーム作成
- 担当者ごとの解決済みモデル、Compatibility、Score、解決理由
- 現在モデル、Runtime、Rotation状態
- Model loading / unloading / reused / completed / failedをActivityへ表示
- ARDカードの視認性改善

## Persistence and Recovery

- Brain Resolution履歴
- Rotation Plan、試行回数、Event、完了状態
- active model / active runtime / active rotation
- 再起動時のInterrupted Rotation検出とPaused復旧
- Resume後はRuntime Adapterが実際のRuntime状態を再確認

## Tests

- `cargo test --workspace`: 75 passed / 0 failed / 5 ignored
- `cargo clippy --workspace --all-targets -- -D warnings`: PASS
- `npm run build`: TypeScript typecheck / Vite production build PASS
- 実Ollama READ ONLY Developer Agent: PASS
- 実Ollama Auto ARD Self Development Loop: PASS
- 実モデル: `qwen3:8b`
- 1モデル環境のためArchitect / Developer / Reviewerで同一ロード済みモデルを再利用
- 実ファイル変更、`cargo check`、`cargo test`、Reviewer承認を確認

## Installer

- File: `InstallPackage/Vertex-AI-0.1.6-x64-setup.exe`
- Size: 35,593,724 bytes
- ProductVersion: 0.1.6
- FileVersion: 0.1.6
- SHA-256: `6d3b907eefe9cd89e5e29ecf814d197e7776ac8da46024b08e6534ac990eaa61`
- Signature: NotSigned

## Known Limitations

- 0.1.6のDesktop Runtime AdapterはOllamaを対象とする。他Runtimeは共通境界へ追加可能。
- Resource PolicyはTyped境界と`Balanced`既定を実装した段階で、全Policyの個別重み調整は次工程。
- GPU VRAMの厳密な予約はRuntimeへ委譲し、Vertex AI側は観測値とCompatibility評価で事前判定する。
- コード署名証明書によるWindows署名は未実施。

## Next Phase

- 複数Runtime AdapterとCloud Model Rotation
- Resource Policyごとの詳細スコアリング
- Model Rotation PerformanceのPostgreSQL集計
- Council / Reviewerを用いたRouting Decision評価
- Signed Installerと自動Release Pipeline

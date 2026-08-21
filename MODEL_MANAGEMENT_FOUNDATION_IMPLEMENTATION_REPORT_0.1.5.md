# Vertex AI 0.1.5 Model Management Foundation 実装報告

## Version

- Vertex AI: `0.1.5`
- 対象OS: Windows 10 22H2以降 / Windows 11 x64
- 基準: 未コミットのVertex AI 0.1.4 ARD Execution Bridge実装
- Git baseline: `0273c35 feat: establish Vertex AI baseline with ARD MVP`
- 注意: 0.1.4以降の正式Baseline Commitは未作成。ユーザー承認なしにCommitは実行していない。

## Architecture

```text
Model Manager UI
    │
    ▼
Tauri Model Management Commands
    │
    ▼
vertex-ai-model-management
    ├─ Typed Model Registry
    ├─ Storage Location Registry
    ├─ GGUF Local Discovery
    ├─ Duplicate Candidate Detection
    ├─ Hardware Snapshot
    ├─ Compatibility Assessment
    ├─ Auto Candidate Ranking Boundary
    ├─ Move Preflight Foundation
    └─ Atomic JSON Persistence / Recovery
         ▲
         │ Discovery Adapter
    LocalRuntimeSnapshot
         ▲
         │
       Ollama
```

ModelとRuntimeを分離し、既存のProvider Registry、Local Runtime Manager、AI Environment Summaryを情報源として再利用した。Ollama固有情報はDiscovery Adapter境界で共通`ModelRecord`へ変換する。

## Implemented Features

### Typed Model Registry

- ModelRecord CRUD
- Model family / format / quantization / parameter size / file size
- Storage location / runtime compatibility
- Local / installed / health / trust / source
- Coding / Reasoning / Review / General / ToolUse / StructuredOutput / LongContext
- RoleとCapabilityの分離

### Model Storage Manager

- 任意フォルダの登録
- 複数保存先
- Default Storage変更
- Path存在、Directory、Write、空き容量の検証
- Duplicate Path拒否
- 親子Path重複拒否
- 外付けStorage切断時もStorage / Model Recordを保持
- Native Folder Picker

### Model Discovery

- 登録StorageのGGUF再帰探索
- Symbolic Linkを追跡しない
- 最大探索Depthを制限
- 巨大ファイル本体を読み込まずFile Metadataのみ取得
- FilenameからFamily / Parameter Size / Quantizationを安全に推定
- Filename + File SizeによるDuplicate候補検出
- 自動削除なし

### Ollama Discovery Adapter

- 既存`LocalRuntimeSnapshot`を共通Model Registryへ取り込み
- Format / Family / Parameter Size / Quantization / Context / Digestを保持
- Ollama停止時は既存Recordを削除せずUnavailableへ遷移
- Runtime復帰時にInventoryを再同期

### Hardware / Compatibility

- Windows System RAM total / availableを実測
- 既存Runtime SnapshotのVRAM in-useを再利用
- 各Storageのtotal / free space
- Compatible / CompatibleWithOffload / ResourceConstrained / Unsupported / Unknown
- 判定理由を構造化して保持
- GPU総VRAMを取得できない環境ではSystem RAM基準へ縮退

### ARD Brain: Auto Boundary

- RoleからRequired Capabilityへ変換
- Model Registry、Hardware Compatibility、Runtime Availabilityから候補を順位付け
- ARD CoreをRegistry内部実装へ埋め込まず、`ModelSelectionRequest` / `ModelCandidate` APIで接続
- 明示Model指定は既存動作を維持

### Model Move Foundation

- Source / Destination / Required BytesのPreflight
- Destination Availability / Free Space確認
- Copy後File Size照合、Registry更新後Cleanup、失敗時Registry非変更の手順をTyped Planとして生成
- 0.1.5では実ファイル移動と削除は行わない

### UI

- 日本語中心、英語切替対応
- Model Manager Hero / Summary
- Installed Models
- Storage Locations
- Hardware
- Capability Tag
- Runtime Compatibility
- This PC Compatibility
- Add Storage / Default Storage / Rescan
- 既存Ollama Download UIをRuntime Adapter領域として維持

## Persistence

- `model-registry-v1.json`
- App Data配下へ保存
- `.next`を利用したAtomic WriteとInterrupted Write Recovery
- Storage Path消失時もアプリ起動を妨げない
- PostgreSQL停止時にもModel Managerを利用可能にするため、0.1.5では既存のJSON縮退Patternを採用

## Security

- Model fileを実行しない
- GGUF全体をメモリへ読み込まない
- Symbolic Linkを追跡しない
- Folder Pickerで選択されたPathをCore側で再検証
- Write Probeは一意な一時ファイルを作成し、直後に削除
- Duplicate候補を自動削除しない
- Move FoundationはPreflightのみで破壊操作なし
- Download / Trust Verificationは次工程

## Primary Files

- `ProgramSource/crates/vertex-ai-model-management/Cargo.toml`
- `ProgramSource/crates/vertex-ai-model-management/src/lib.rs`
- `ProgramSource/apps/management-ui/src/services/model-management.ts`
- `ProgramSource/apps/management-ui/src-tauri/src/lib.rs`
- `ProgramSource/apps/management-ui/src/App.vue`
- `ProgramSource/apps/management-ui/src/style.css`
- `ProgramSource/apps/management-ui/src-tauri/capabilities/default.json`
- `ProgramSource/apps/management-ui/src-tauri/Cargo.toml`
- `ProgramSource/apps/management-ui/package.json`
- `ProgramSource/Cargo.toml`

## Tests

### Deterministic Model Management Tests

7 / 7 PASS

- Storage CRUD / Default / Persistence
- Invalid Path / Duplicate / Parent-Child overlap
- Unavailable Storage / Model Recovery
- GGUF Discovery / Metadata / Duplicate Candidate
- Ollama Inventory Reconciliation
- Model CRUD / Compatibility / Auto Candidate Ranking
- Move Preflight non-mutation

### Workspace Regression

- `cargo test --workspace --quiet`: 69 PASS / 0 FAIL / 5 environment-specific ignored
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: PASS / warning 0
- `vue-tsc -b`: PASS
- `vite build`: PASS

### Real Ollama Regression

- `actual_ollama_read_only_developer_agent_acceptance`: PASS (11.01 sec)
- `actual_ollama_ard_self_development_loop_acceptance`: PASS (35.03 sec)
- Architect → Developer → cargo check/test → Reviewer承認を実Ollamaで完走

### Release

- Tauri Release: PASS
- NSIS: PASS
- File Version: `0.1.5`
- Product Version: `0.1.5`
- Size: `35,583,639 bytes`
- SHA-256: `54aa221bbdfe817e1a5df6a3bafc336583b4a164c93e2a2e50b9f60046dafaa2`
- Code Signature: NotSigned

## Installer

`InstallPackage/Vertex-AI-0.1.5-x64-setup.exe`

## Known Issues / Limitations

- GPU総VRAM / 利用可能VRAMのvendor-neutral検出は未実装。現在はRuntimeのVRAM使用量とSystem RAMで縮退判定する。
- GGUF内部Header Metadataは解析せず、FilenameとFilesystem Metadataを使用する。
- Model Moveは安全なTyped Preflightまで。Copy / Verification / Cleanup Jobは次工程。
- Model RegistryのPostgreSQL Repository実装は未追加。Memory Runtime障害から独立するAtomic JSONを0.1.5の永続化に採用した。
- Model Download Verification / Trust Promotion / Hugging Face Searchは対象外。
- インストーラーはコード署名されていない。

## Next Phase

1. Model Search / Download / Verification / Install
2. GPU AdapterによるVRAM total / available検出
3. Brain: AutoのUI選択と実運用Telemetry
4. Runtime Load / Unload
5. Single GPU Model Rotation
6. Model Move JobのCopy / Verification / Rollback
7. Built-in Local Inference Runtime

## Completion Result

Vertex AI 0.1.5は、複数Storageを管理し、Local GGUFとOllama Modelを共通Registryで把握し、Capability、Runtime Compatibility、Hardware CompatibilityをUIへ表示できるModel Management Foundationとして成立した。既存Developer AgentおよびARD 0.1.4の実Ollama Self Development Loopは回帰していない。

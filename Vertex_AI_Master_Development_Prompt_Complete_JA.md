# Vertex AI — マスター開発プロンプト【完全版・日本語】
## 完全再構成仕様 / Codex 実装指示書

> 状態: マスター仕様
> 目的: Vertex AI の開発方針を、途中追記ではなく最初から一貫した「唯一の基準文書」として再構成する。
> 原則: 既存の追記仕様と矛盾する場合、本書の設計思想を優先する。ただし実装は段階的かつ安全に進める。

---

# 1. ミッション

**Vertex AI** を単なるチャットアプリとして作ってはならない。

Vertex AI は、**ローカルファーストのAIコントロールプレーン**であり、同時に**コンピューター環境を理解・管理する知的環境マネージャー**として構築する。

Vertex AI は、人間のユーザーと以下のものの間に位置する。

- ローカルAIモデル
- クラウドAIモデル
- AIランタイム
- API / Provider
- アプリケーション
- 開発者向けツール
- クリエイター向けツール
- OSリソース
- ハードウェア
- ストレージ
- 将来のVertex製品群

ユーザーが、モデル・ランタイム・Provider・API・PATH・サービス・レジストリ・GPUバックエンド・保存場所などの違いを理解していなくても、Vertex AIがそれらを理解して橋渡しする。

Vertex AI のUX上の約束は次の通り。

> **ユーザーは「何をしたいか」を伝える。Vertexは、このPCに何があり、何ができ、何が不足し、何が壊れていて、どう進めるのが最も安全かを理解する。**

Vertex AI は次の2つを兼ねる。

1. **AIオーケストレーション / コントロールプレーン**
2. **ホストPCそのものを理解する知的環境レイヤー**

さらに、その機能は他のVertex製品から再利用できる共通基盤として設計する。

---

# 2. 製品哲学

## 2.1 人間中心の抽象化

AIを利用するだけのために、一般ユーザーへインフラ知識を要求してはならない。

内部では以下を扱ってよい。

- ローカルLLMランタイム
- クラウドAPI
- モデル形式
- Embedding
- Context Window
- CUDA
- GPUドライバー
- 環境変数
- Windowsサービス
- レジストリ
- ポート
- ストレージパス
- 依存関係
- データベース
- クリエイターアプリ
- 開発ツール

しかしユーザーには、これらを**理解できる状態と操作**へ翻訳して提示する。

例。

悪い表示:

> ECONNREFUSED 127.0.0.1:11434

望ましい表示:

> Ollamaが現在のAI Providerとして設定されていますが、このPC上でOllamaが動作していることを確認できませんでした。
> OpenAIは代替Providerとして利用可能です。
>
> [Providerを切り替える] [Ollamaを確認する] [技術情報]

ただし、生のエラーコードや技術情報は「詳細」「Developer View」などから必ず確認可能にする。

## 2.2 事実・推論・提案・実行を分離する

Vertexは以下を明確に区別する。

- **観測された事実** — システムから直接確認できた状態
- **推論** — 証拠からAIが推定した原因や意味
- **推奨** — 次に取るべきとVertexが考える行動
- **変更** — 実際にPCへ加える操作

AIの推論を「確認済みの事実」として表示してはならない。

また、LLMが提案したという理由だけで高リスクな変更を実行してはならない。

## 2.3 原則として元に戻せること

修復・移行・クリーンアップ・設定変更・モデル移動は、技術的に可能な限り可逆的にする。

利用する仕組み:

- バックアップ
- Dry Run
- 変更プレビュー
- 必要に応じた復元ポイント
- ロールバック情報
- 監査ログ
- トランザクション的な処理

## 2.4 Local-first / Provider-neutral

Vertex AIを単一のAI企業やモデルに依存させない。

基本原則:

- Local-first
- LLM-agnostic
- Provider-neutral
- Memory-centric
- Transport-agnostic
- Edge-resilient
- VXN-ready
- 他Vertex製品から再利用可能

クラウドAIは選択肢であり、アーキテクチャ上の必須依存ではない。

---

# 3. 製品としての位置づけ

Vertex AIを一般的なチャットAIアプリとして設計しない。

主役は**管理・理解・診断・制御のコンソール**である。

主要画面の概念:

- Dashboard
- Models
- Providers
- Routing
- Memory
- Applications
- Environment Explorer
- System Health
- Storage
- Edge Cores
- Security
- System Status
- Logs
- AI Test Console

会話型AIは操作インターフェースの一つとして存在してよいが、システム全体をチャット中心の設計にしてはならない。

---

# 4. 基準技術スタック

明確な技術的理由がない限り、以下を基準とする。

### Desktop / UI
- Tauri 2
- Vue 3
- TypeScript
- Vite
- Pinia
- Quasar、または同等に保守性の高いUIコンポーネント層

### Core
- Rust

### 永続メモリ / 構造化状態
- 適切な箇所で PostgreSQL

### Security
- APIキーやCredentialはOSネイティブのSecret Storageを使用
- 通常の平文設定ファイルへ秘密情報を保存しない

UIから直接OS固有の探索処理を行わず、安定したCore API / Eventを介する。

---

# 5. Vertex Edge Core

再利用可能なRustサービス層として **Vertex Edge Core** を構築する。

Edge Coreは、LLMに任せるべきではない**決定論的・高速・低レベル処理**を担当する。

責務:

- ファイルシステム探索
- 永続インデックス
- 差分スキャン
- File System Watcher
- ファイルHash / Fingerprint
- 重複検出
- モデル探索
- Runtime探索
- アプリケーション探索
- 依存関係探索
- ストレージ監視
- Process / Service Health
- ハードウェア探索
- GPU / VRAM / RAM情報
- Model Registry
- Migration Job
- Background Job Queue
- 診断情報収集
- 安全なシステム検査
- 構造化イベント発行

**決定論的コードで事実を確認できる場所をAIで代替しない。**

Rustが証拠を集め、AIがその意味を理解する。

---

# 6. Provider Architecture

すべてのAI Backendを共通Provider抽象化の下へ置く。

対象例:

- OpenAI互換API
- Ollama等のローカルRuntime
- LM Studio互換Endpoint
- その他ローカルEngine
- 将来のCloud Provider
- Vertex Native Provider

全Providerを無理やり同一仕様にするのではなく、**Capabilityベース**で扱う。

Capability例:

- Chat
- Streaming
- Tool Calling
- Structured Output
- Embeddings
- Vision
- Audio
- Context Limit
- Model Listing
- Health / Status
- Authentication
- Local / Cloud

新Provider追加時にVertex本体を書き換える構造を避ける。

基本:

> Adapter / Plugin + Configuration + Capability Declaration + Registration

---

# 7. BYOK / Credential

**Bring Your Own Key (BYOK)** を正式サポートする。

基本フロー:

1. Providerを選択
2. Credentialを入力
3. OS保護Secret Storageへ保存
4. 接続検証
5. 対応モデル / Capabilityを探索
6. Routingから利用可能にする

保存済みSecretを不用意に表示しない。

ログにはCredentialを出さない。

---

# 8. Model Routing

最低限、次の3モードを概念として持つ。

## Manual
ユーザーがProvider / Modelを明示選択する。

## Auto
VertexがCapability、可用性、Privacy、Cost、Performance、Context Size、Task Requirement等から適切なモデルを選ぶ。

## Council
必要なワークフローでは複数モデルが独立して推論・評価し、Coordinatorが統合または判定する。

RoutingはハードコードではなくPolicy-drivenにする。

Local ModelとCloud Modelを同じOrchestration Layerの対等な選択肢として扱う。

---

# 9. Model Manager

統合 **Model Manager** を構築する。

ユーザーが行えること:

- モデル探索
- 既存モデル登録
- 対応Runtime / Provider経由のダウンロード
- Format / Metadata確認
- モデル選択
- Compatibility確認
- RAM / VRAM / Storage要件確認
- 対応していればUpdate
- 安全な削除
- モデルファイル所在地確認
- Duplicate検出
- Drive間移動
- どのRuntimeがどの物理データを利用可能か確認

システムドライブを当然の保存先と仮定しない。

Cドライブ以外のDドライブ等をモデル保存先として自由に選べるようにする。

VertexがStorage Rootを一元管理する。

---

# 10. Shared Model Library / Duplicate Prevention

同一の巨大モデルをユーザーが知らないうちに何度も保存する問題を防ぐことを主要目標とする。

検出対象:

- 同一モデルファイル
- Runtimeごとに異なるLayoutで保存された同等モデル
- Duplicate GGUF
- Runtime管理Blob / Manifest
- Orphaned Model Data
- Cache
- 中断・古いDownload

必要に応じてHash / FingerprintとMetadataを組み合わせる。

ただし、

**OllamaとLM Studioなど、異なるRuntimeが常に同一物理ファイルを直接共有できると約束してはならない。**

分類例:

- 直接共有可能
- 再ダウンロードなしでImport可能
- Convert可能
- Runtime専用Copyが必要
- 不明 / 未対応

理由もユーザーへ説明する。

---

# 11. Storage & Migration Wizard

安全な **Storage & Migration Wizard** を提供する。

例:

> システムドライブの空き容量が少なくなっています。
> C: に38GBのローカルAIモデルがあります。
> D: には十分な空き容量があります。

操作候補:

- 選択モデルを移動
- Default Model Storage変更
- Cache移行
- Runtime Compatibility維持
- コピー後のValidation
- Validation成功後に参照先更新
- 失敗時Rollback

**移行先の検証が終わる前に元データを削除しない。**

---

# 12. Vertex Environment Explorer

AI専用Explorerを超えて、より広い **Vertex Environment Explorer** とする。

答えるべき問い:

> **このコンピューターに何がインストールされ、どこにあり、何をするもので、それによってVertexは何ができるのか？**

AIツールだけに限定しない。

分類例:

- AI
- Developer
- Creator
- Runtime
- Database
- Server
- System
- Hardware
- Storage

---

# 13. Developer Environment Discovery

可能な範囲で開発環境を検出・分類する。

例:

- Python
- Node.js
- Rust Toolchain
- Git
- VS Code
- Visual Studio
- Compiler
- Package Manager
- Docker / Container Tool
- Database Client / Server
- SDK
- CUDA / Toolkit
- AI Runtime

保持するMetadata例:

- Executable Path
- Version
- Architecture
- Environment Registration
- Active / Default Version
- 関連Service
- 関連PATH

---

# 14. Creator Environment Discovery

クリエイター向けアプリケーションもVertexが理解する。

Capability分類例:

- 画像編集
- Vector Graphics
- 動画編集
- Compositing
- 3D制作
- 音声編集
- Streaming / Recording
- Media Conversion

対象例:

- Photoshop
- Illustrator
- Premiere Pro
- After Effects
- DaVinci Resolve
- Blender
- Affinity系アプリ
- OBS
- Audacity
- ffmpeg
- 将来のCreator Tool

単にアプリ一覧を表示するだけでは不十分。

**Capability Graph** を構築する。

例:

- Blender -> 3D Modeling / Rendering
- DaVinci Resolve -> Video Editing / Color / Audio
- ffmpeg -> Media Conversion / Transcoding
- VS Code -> Source Editing / Development

これにより、ユーザーの依頼を「このPCが既に持つ能力」と結びつけられる。

例:

> 「この動画を軽くして」

Vertexはffmpegが既に存在すると判断し、別アプリを無駄にインストールするのではなく、それを利用する選択肢を提案できる。

---

# 15. 高速Environment Search

Environment Discoveryは非常に高速であること。

Everythingのような高速インデックス型検索ツールの**ユーザー体験や設計思想**は参考にしてよいが、独自実装を安易にコピーしない。

設計:

- Persistent Index
- Initial Scan
- Incremental Update
- File System Watching
- Target Root
- Exclusion
- Cancellation
- Low-priority Background Scan
- Fast Metadata Query
- Hash Jobを軽量Discoveryから分離

毎回全Driveを最初からスキャンしない。

Hashは必要な時だけ行う。

---

# 16. AI Asset Search

インデックス化された環境に対してSemantic Searchを提供する。

例:

- 「GGUFモデルはどこ？」
- 「Pythonを全部見せて」
- 「動画編集できるアプリは？」
- 「AIモデルで40GB使っているのは何？」
- 「Ollamaの残骸はまだある？」
- 「重複モデルを探して」
- 「開発ツールは何が入ってる？」
- 「このPCでこのモデル動く？」

結果は、決定論的なIndex FactとAIによる説明を組み合わせる。

---

# 17. System Health / Environment Doctor

**Vertex System Health / Environment Doctor** を構築する。

現在インストールされているアプリだけを見るのではなく、インストール・Update・Uninstall後に残った壊れた環境状態まで調べる。

対象:

- Application Install Record
- File / Folder残骸
- Configuration File
- Environment Variable
- PATH
- Startup Entry
- Service
- Scheduled Task
- Port
- Runtime Reference
- Model Reference
- Cache
- Windows Registry
- 古いProvider Endpoint
- 存在しないExecutable Path

例: OllamaをUninstall済みの場合。

Vertexは以下のように報告できる。

- Ollama Executable: 未検出
- Ollama Service: 未検出
- Model Data: 6GB残存
- Configuration: 残存
- PATH / Reference: 古い参照あり
- 他アプリの設定: Ollama Endpointをまだ参照
- Cloud Provider: 利用可能

単なる「Ollama not found」よりはるかに有用な診断にする。

---

# 18. Registry Safety

Windows Registryを診断対象にしてよい。

ただしVertexを**攻撃的なRegistry Cleanerにしてはならない。**

Vertexがすぐに正体を特定できないという理由だけでRegistry Entryを削除してはならない。

Cleanup提案前に可能な限り調べる:

- Ownership
- 参照Executable / Component
- Shared Component Usage
- COM Relationship
- Service / Driver Relationship
- Installer Metadata
- Current Reference
- OrphanであるConfidence

分類例:

- Verified Orphan
- Likely Orphan
- Unknown Ownership
- Shared / System Component
- Protected / Do Not Modify

曖昧なEntryやSystem CriticalなEntryの自動削除は禁止。

Diagnosis、Explanation、Reversible Remediationを優先する。

---

# 19. Health State

明確な状態を用いる。

- Ready
- Warning
- Offline
- Misconfigured
- Missing Dependency
- Conflict Detected
- Orphan Detected
- Repair Available
- Unknown

一目で利用可能か理解できるUIにする。

---

# 20. Smart Fix & Guidance

**Smart Fix & Guidance** を構築する。

Repair提案ごとの基本フロー:

1. Vertexが何を発見したか説明
2. FactとInferenceを分離
3. なぜ問題なのか説明
4. 何を変更する予定か表示
5. Risk Level表示
6. 必要ならDry Run
7. 意味のある変更には明示承認
8. 可能ならBackup / Recovery
9. Deterministic Coreが実行
10. 結果をVerification
11. Action Log記録
12. 対応可能ならRollback提供

**LLMは提案する。Trusted Execution Layerが検証して実行する。**

---

# 21. Vertex AI Error Intelligence

無機質な定型エラーダイアログを **Vertex AI Error Intelligence** へ進化させる。

エラー発生時、必要な範囲に限定して診断Contextを収集する。

例:

- 失敗したOperation
- Raw Error / Error Code
- Application / Module
- Provider
- Model
- Runtime
- Process / Service Status
- Dependency Status
- 関連Configuration
- 関連Recent Log
- Environment Explorer Evidence
- 直前のUser Action

そしてユーザーへ次の構造で伝える。

### 何が起きたか
人間が理解できる説明。

### Vertexが確認できたこと
Verified Fact。

### 考えられる原因
AI Inferenceであることを明示。

### できること
安全な選択肢。

### 技術情報
Raw Error、Log、Stack Trace、ID、Developer Data。

例:

> **選択されたAI Runtimeへ接続できませんでした。**
>
> 確認済み: Ollamaが選択されていますが、現在Ollama Service / Executableを検出できません。
>
> 推定原因: Ollamaを削除した後も古いProvider設定が残った可能性があります。
>
> OpenAIへの接続は利用可能です。
>
> [OpenAIへ切り替える] [Ollama設定を修復] [技術情報]

---

# 22. Predictive Error Prevention

Error Intelligenceを最終的には**予防型**にする。

将来失敗しそうなConfigurationを検出した場合、実際のErrorになる前に警告できる。

例:

> この設定は現在存在しないExecutableを参照しています。次回のモデル起動時に失敗する可能性があります。

ただし推測警告を乱発しない。

ConfidenceとSeverity Thresholdを設ける。

---

# 23. AI時代のDevice Manager

管理UIは、**AI時代のデバイスマネージャー**のように機能させる。

一目で理解できる内容:

- どのAI Systemがあるか
- どのModelがあるか
- Modelがどこに保存されているか
- どのRuntimeが起動するか
- どのProviderがActiveか
- 各ComponentはHealthyか
- どのApplication / Toolが利用可能か
- このPCにはどんなCapabilityがあるか
- 何がDuplicateか
- 何が壊れているか
- 何を安全にRepairできるか

これはVertex AIの主要な製品アイデンティティとする。

---

# 24. Computer Capability Graph

PC内部を構造化されたEntityとRelationshipとして表現する。

Entity例:

- Application
- Executable
- Runtime
- Provider
- Model
- Model File
- Service
- Process
- Port
- Dependency
- SDK
- Driver
- GPU
- Storage Device
- Registry Entry
- Environment Variable
- Configuration
- Capability

Relationship例:

- installed_at
- executes
- depends_on
- provides
- references
- stores
- launches
- listens_on
- compatible_with
- duplicates
- supersedes
- orphaned_from

これにより、分野をまたいだDiagnosisを可能にする。

---

# 25. Memory

Vertex AIはMemory-centricとする。

ただし、すべてを一つのVector Storeへ放り込まない。

分離する:

- User / Project Memory
- System / Environment State
- Provider / Model Metadata
- Verified Knowledge
- Temporary Conversation Context
- Log / Audit Record

構造化情報には適切にPostgreSQLを使用し、Embedding / RAGは価値がある場所だけに使う。

---

# 26. Knowledge Core / 継承

Vertexは**元の開発者がいなくなってもプロジェクトが存続できる**よう設計する。

**Vertex Knowledge Core** を構築し、以下を保存する。

- Product Philosophy
- Architecture Decision
- なぜそのDecisionをしたのか
- Coding Convention
- Security Principle
- 採用しなかったAlternativeと理由
- Component Responsibility
- Compatibility Promise
- Build / Deployment Procedure
- Recovery Procedure
- Terminology
- Roadmap Intent
- Known Technical Debt

目的は単なるDocument保管ではない。

目的は**開発思想の継承**である。

将来のMaintainerが、

> 「なぜProvider抽象化をこの形にしたの？」

と尋ねた時、AIの想像ではなくVersion管理されたProject Recordに基づいて答えられるようにする。

---

# 27. Fact Preservation / Provenance

長期保存するKnowledgeでは、**Source FactとInterpretationを分離する。**

重要なFactには可能な限り以下のProvenanceを保持する。

- Source
- Author / System
- Timestamp
- Version
- Commit / Release
- Confidence
- 必要に応じたOriginal Text / Hash

AI SummaryがSource Recordを上書きしてはならない。

Originalを保存し、InterpretationはDerived Layerとして生成する。

この原則はProject Historyだけでなく、将来Vertexが「AI語り部」「Archive」的な役割を担う場合にも適用する。

> **事実は記録。解釈は、その記録に対するViewである。**

---

# 28. Versioned Architecture Decision

ADR、または同等の構造化記録を利用する。

Major Decisionごとに保存:

- Context
- Decision
- Alternatives
- Rationale
- Consequences
- Date / Version
- 変更された場合のSuperseding Decision

将来の人間とAI Maintainerが信頼できる開発系譜を残す。

---

# 29. UI / UX Principles

LM Studio等の「分かりやすさ」「発見しやすさ」は参考にしてよい。

ただしVisual Designを模倣しない。

Vertex独自の一貫したDesign Languageを持つ。

UX Priority:

- Current Modelが見える
- Current Providerが見える
- Healthが見える
- Storageが見える
- Model / Provider / Runtime / APIの違いが理解できる
- Progressive Disclosure
- 初心者向けの言葉
- 必要ならDeveloper Detailを表示
- 危険操作は明確に区別
- Error Codeだけを主メッセージにしない
- 通常操作にTerminalを要求しない

AI環境管理のためにユーザーがVertexの外へ出る必要を極力減らす。

---

# 30. Security Model

Least Privilegeを採用する。

原則:

- Inspectionは本当に必要でない限り管理者権限を要求しない
- Elevationは具体的なPrivileged Actionの直前だけ
- AI生成Shell Commandを無検証で実行しない
- Sensitive MutationはAllowlist / Typed Operationを使う
- SecretをLogへ出さない
- Administrative Changeを記録
- External Toolは可能な範囲でSandbox / Constraint
- Read-only DiagnosisとMutation Permissionを分離

---

# 31. Performance

大規模なDeveloper / Creator PCでもUIをResponsiveに保つ。

要求:

- Background Job
- Bounded Concurrency
- Cancellable Operation
- Incremental Index
- Cached Metadata
- Debounced File Event
- Lazy Hashing
- Priority Scheduling
- UI Progress
- UI ThreadをBlockしない
- 巨大Log / Fileを不用意にLLMへ送らない

AIを、決定論的に高速処理できるHot Pathへ入れない。

---

# 32. Offline / Degraded Operation

Cloud Providerが利用不能でもGracefulに動作する。

- Local Modelがあれば利用可能
- Environment Inspectionは利用可能
- Deterministic Diagnosisは利用可能
- Cached Metadataは利用可能

設定されたProviderが消えた場合:

- 無限Failure Loopに入らない
- Missing Provider / Runtimeを特定
- 人間向けに説明
- 利用可能なAlternativeを提示

---

# 33. Extensibility

Extension Pointを用意する。

- Provider
- Runtime
- Model Format
- Scanner
- Capability Classifier
- Health Check
- Repair Action
- Application Integration
- Vertex Product

Product Nameごとの巨大Switch文を避ける。

Registry、Adapter、Typed Capability、Schema、Versioned Interfaceを優先する。

---

# 34. Vertex Ecosystemとの統合

Vertex AIはVertex製品群の共通知能・Control Layerである。

全体の方向性:

> **Think -> Design -> Build -> Run -> Publish -> Sell -> Operate**

将来のVertex製品はVertex AIへ以下のような質問ができる。

- 利用可能なModelは？
- このTaskはどのProviderが適切？
- Local AI RuntimeはHealthy？
- 動画編集アプリは入っている？
- このPCでこのModelは動く？
- 利用可能なDatabase / Runtimeは？
- このErrorを引き起こしたEnvironment Problemは？

一つのDownstream Productへ密結合しない。

---

# 35. Implementation Strategy

すべてを同時実装しない。

Architectureを壊さずVertical Sliceで積み上げる。

## Phase 0 — Foundation
- Repository Structure
- Rust Core
- Tauri / Vue Shell
- Typed IPC / API Contract
- Configuration
- Secure Secret Abstraction
- Logging / Audit Foundation

## Phase 1 — Provider + Model Minimum
- Provider Interface
- Cloud Provider 1つ
- Local Provider / Runtime 1つ
- Health Check
- Model Registry
- Manual Routing
- AI Test Console

## Phase 2 — Environment Explorer
- File / Application Discovery
- Runtime Detection
- Developer Tool Detection
- Persistent Index
- Fast Search
- Storage Inventory

## Phase 3 — Model Storage Intelligence
- Model Fingerprint
- Duplicate Detection
- Storage Root
- Compatibility Classification
- Migration Dry Run
- Safe Migration

## Phase 4 — System Health
- PATH / Environment Diagnosis
- Stale Endpoint Detection
- Service / Startup / Task Inspection
- Windows Registry Read-only Diagnosis
- Orphan Confidence Model

## Phase 5 — Error Intelligence
- Structured Error Envelope
- Diagnostic Context Collection
- Fact / Inference Separation
- AI Explanation
- Guided Repair Plan

## Phase 6 — Creator Capability Graph
- Application Classification
- Capability Mapping
- 「このPCで何ができる？」Semantic Query
- Tool Selection Recommendation

## Phase 7 — Smart Fix
- Typed Repair Action
- Preview
- Permission / Elevation Boundary
- Backup / Rollback
- Verification

## Phase 8 — Knowledge Core
- ADR Ingestion
- Architecture Knowledge
- Provenance
- Versioned Project Philosophy
- Project RecordにGroundしたMaintainer Q&A

## Phase 9 — Auto / Council Routing & Advanced Orchestration
- Routing Policy
- Task Capability Matching
- Cost / Privacy / Performance Policy
- Multi-model Workflow

---

# 36. 初期Data Contract

早い段階で安定したTyped Entityを定義する。

候補:

```text
Provider
ProviderCapability
Model
ModelArtifact
Runtime
Application
ApplicationCapability
Dependency
EnvironmentFinding
HealthCheck
RepairPlan
RepairAction
StorageRoot
MigrationJob
SystemAsset
SystemRelationship
ErrorEvent
DiagnosticContext
KnowledgeRecord
ArchitectureDecision
AuditEvent
```

各EntityはStable IDとVersion可能なSchemaを持つ。

UI専用StructureをCanonical Domain Modelにしない。

---

# 37. Error Contract

各Subsystemは自由形式文字列ではなくStructured Error Envelopeを返す。

概念:

```text
error_id
timestamp
component
operation
severity
machine_readable_code
human_fallback_message
technical_message
causes[]
evidence_refs[]
suggested_check_ids[]
recoverable
retryable
```

Error Intelligenceが後からEnrichしてよいが、Raw Evidenceは保持する。

---

# 38. Repair Contract

RepairはAIが作った任意Scriptではなく、Typed Planとして扱う。

概念:

```text
repair_plan_id
finding_ids[]
summary
risk
requires_elevation
requires_restart
reversible
backup_strategy
actions[]
verification_steps[]
rollback_actions[]
```

AIはCandidate Planを構築・選択できる。

Coreは実行前にSupported OperationかValidationする。

---

# 39. Non-Goals / 禁止事項

以下を行わない。

- Vertex AIを単なるChat UIにする
- Ollama / LM Studio / OpenAI等の一社・一RuntimeへHard-wireする
- Registry EntryをSilent Deleteする
- Model FileをSilent Deleteする
- User DataをSilent Relocateする
- LLM生成Shell CommandをAdministrator権限で無検証実行する
- 確認していないのにRuntime間でModel File共有可能と表示する
- 不要なFull Disk Rescan
- API KeyのPlaintext保存
- Advanced UserからRaw Technical Detailを隠す
- AI GuessをFactとして表示
- AI SummaryでHistorical Source Recordを上書き
- UIにOS Discovery Logicを持たせる
- 通常操作でCommand Lineを必須にする

---

# 40. Definition of Success

Vertex AIを起動したユーザーが短時間で次を理解できること。

1. どんなAI Modelを持っている？
2. どこに保存されている？
3. Duplicateはある？
4. どのProvider / Runtimeを使っている？
5. 正常に動いている？
6. 動かないならなぜ？
7. Vertexが安全に直せる？
8. Developer / Creator Toolは何が入っている？
9. それらを使ってこのPCは何ができる？
10. Missing Dependencyは？
11. 削除済みSoftwareの残骸が問題を起こしていない？
12. 巨大AI Assetを別Driveへ安全に移せる？
13. 他のVertex ProductがこのIntelligenceを再利用できる？
14. 将来のMaintainerが「なぜこの設計なのか」を理解できる？

理想体験:

> **Vertexがコンピューターを理解する。だからユーザーは、AIを使うためだけにシステムエンジニアになる必要がない。**

---

# 41. Codex 実装指示

この仕様を実装する際は:

1. Architecture変更前に既存Repositoryを調査する。
2. 本仕様が明示的に置き換えない限り、動作中の機能を壊さない。
3. 現行CodeとMaster DesignのConflictを特定する。
4. 最小かつ一貫したImplementation Sliceを提案する。
5. Domain LogicをUIへ置かない。
6. Typed Rust InterfaceとStable Schemaを優先する。
7. Discovery / Classification / Migration / Health Check / Repair ValidationにTestを追加する。
8. OS MutationはSecurity-sensitiveとして扱う。
9. Windows固有処理はPlatform Abstractionの後ろへ置く。
10. Architecture Decisionを実装と同時にDocument化する。
11. Architecture Behavior変更時はKnowledge Core / ADRを更新する。
12. System Stateが変わったと決めつけず、Mutation後は必ずVerificationする。
13. 巨大RewriteよりIncremental / Compilable / TestableなCommitを優先する。

Feature実装前に内部的に必ず問う:

> **これは「決定論的な事実収集」か、「AIによる解釈」か、それとも「権限を伴う変更」か？**

そして正しいLayerへ配置する。

---

# 42. Core Design Maxim — Vertex AIの設計原則

最終的なArchitecture判断では、常にこの原則へ戻る。

> **Observe deterministically. Understand intelligently. Explain humanly. Act safely. Preserve the reason why.**

日本語では:

> **決定論的に観測する。知的に理解する。人間に分かる言葉で説明する。安全に行動する。そして「なぜそうしたのか」を残す。**

これをVertex AIの根幹とする。

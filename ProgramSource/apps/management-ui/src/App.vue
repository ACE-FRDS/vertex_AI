<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import {
  Activity,
  AlertTriangle,
  AppWindow,
  Bell,
  Blocks,
  Bot,
  BrainCircuit,
  Check,
  ChevronDown,
  ChevronRight,
  CircleGauge,
  Clock3,
  Cloud,
  Code2,
  Command,
  Cpu,
  Database,
  FlaskConical,
  KeyRound,
  Languages,
  HardDrive,
  LayoutDashboard,
  ListFilter,
  MemoryStick,
  Menu,
  MoreHorizontal,
  Network,
  Plus,
  Power,
  RefreshCw,
  Route,
  Search,
  Settings,
  ShieldCheck,
  Sparkles,
  TerminalSquare,
  X,
  Zap,
} from 'lucide-vue-next'
import { getCachedEnvironment, scanEnvironment, type EnvironmentSnapshot, type SystemAsset } from './services/environment'
import {
  generateLocal,
  cancelModelDownload,
  getAiEnvironment,
  getLocalProviderStatus,
  listModelDownloads,
  startModelDownload,
  unloadLocalModel,
  type AiEnvironmentSummary,
  type GenerationResponse,
  type LocalProviderStatus,
  type ModelDownloadJob,
} from './services/local-ai'
import {
  diagnoseMemoryCore,
  getMemoryCoreStatus,
  isDesktopRuntime,
  restartMemoryCore,
  searchSystemMemories,
  startMemoryCore,
  stopMemoryCore,
  storeSystemMemory,
  type MemoryCoreStatus,
  type MemoryRecord,
  type RuntimeDiagnosis,
} from './services/memory'
import {
  cancelDeveloperTask,
  getDeveloperTask,
  listDeveloperWorkspaces,
  registerDeveloperWorkspace,
  rollbackDeveloperTask,
  startDeveloperTask,
  type DeveloperMode,
  type DeveloperTask,
  type DeveloperWorkspace,
} from './services/developer'
import {
  cancelArdSession,
  createArdTeam,
  createArdWorkflow,
  getArdSession,
  listArdSessions,
  listArdTeams,
  listArdWorkflows,
  pauseArdSession,
  resumeArdSession,
  startArdSession,
  type ArdSession,
  type ArdTeam,
  type ArdWorkflow,
} from './services/ard'
import {
  addModelStorageLocation,
  chooseModelStorageDirectory,
  getModelManagementSnapshot,
  scanModelStorage,
  setDefaultModelStorage,
  type ModelManagementSnapshot,
  type ModelRecord,
} from './services/model-management'

type Locale = 'ja' | 'en'
type PageKey =
  | 'Dashboard'
  | 'Models'
  | 'Providers'
  | 'Memory'
  | 'Routing'
  | 'Applications'
  | 'AI Environment'
  | 'Local Runtimes'
  | 'Environment Explorer'
  | 'Storage'
  | 'Edge Cores'
  | 'Security'
  | 'System Status'
  | 'Logs'
  | 'AI Test Console'
  | 'Developer Agent'
  | 'Settings'
type GroupKey = 'intelligence' | 'runtime' | 'operations' | 'developer'
type NavItem = { key: PageKey; icon: typeof LayoutDashboard; group?: GroupKey }

const messages = {
  ja: {
    nav: {
      Dashboard: 'ダッシュボード', Models: 'モデル', Providers: 'プロバイダー', Memory: 'メモリ', Routing: 'ルーティング',
      Applications: 'アプリケーション', 'AI Environment': 'AI環境マネージャー', 'Local Runtimes': 'ローカルランタイム', 'Environment Explorer': '環境エクスプローラー', Storage: 'ストレージ', 'Edge Cores': 'エッジコア', Security: 'セキュリティ', 'System Status': 'システム状態',
      Logs: 'ログ', 'AI Test Console': 'AIテストコンソール', 'Developer Agent': 'Developer Agent', Settings: '設定',
    },
    groups: { intelligence: 'インテリジェンス', runtime: 'ランタイム', operations: '運用', developer: '開発者' },
    descriptions: {
      Dashboard: 'システム全体とインテリジェンス処理の概要', Models: '登録済み推論エンジンと機能', Providers: 'プロバイダー接続とモデル検出',
      Memory: '永続的な経験・スコープ・検索', Routing: 'モデル選択ポリシーとルーティング判断', Applications: 'Vertex AI Coreに接続された製品', 'AI Environment': 'ローカルAIランタイム、モデル、使用量を一元管理', 'Local Runtimes': 'ローカル推論エンジンの稼働状況とメモリ使用状態', 'Environment Explorer': 'このコンピューターのツール、ランタイム、機能を検出', Storage: 'モデル、キャッシュ、保存先、空き容量を管理',
      'Edge Cores': 'ローカル自律動作とオフライン運用', Security: 'シークレット、権限、プライバシー境界',
      'System Status': 'Coreサービスの状態と性能', Logs: '非公開情報を含まない運用イベント',
      'AI Test Console': 'プロバイダーとコンテキストの開発者向け診断', 'Developer Agent': 'リポジトリを安全に解析・編集・検証する自律型開発エージェント', Settings: 'Coreの動作と環境設定',
    },
    closeNavigation: 'ナビゲーションを閉じる', primaryNavigation: 'メインナビゲーション', openNavigation: 'ナビゲーションを開く',
    coreOnline: 'Core オンライン', localVersion: 'v0.1.6 · ローカル', administrator: '管理者', localWorkspace: 'ローカルワークスペース',
    searchGlobal: 'モデル、メモリ、設定を検索…', previewData: 'プレビューデータ', notifications: '通知', localCore: 'ローカルCore',
    management: 'Vertex AI 管理コンソール', refresh: '更新', addProvider: 'プロバイダーを追加',
    thisWeek: '今週 +2', availableModels: '利用可能なモデル', connectedProviders: '2プロバイダー接続済み',
    memoryRecords: 'メモリレコード', retrievalPrecision: '検索精度 98.7%', last24h: '過去24時間', aiOperations: 'AI処理',
    servedLocally: '64%をローカル処理', healthy: '正常', degraded: '低下', coreAvailability: 'Core稼働率', commandLatency: 'コマンド遅延 28 ms',
    intelligenceActivity: 'インテリジェンス処理', routedByLocation: '実行場所別のリクエスト', last7Days: '過去7日間',
    localModels: 'ローカルモデル', cloudModels: 'クラウドモデル', activityChart: 'ローカルおよびクラウド処理グラフ',
    aug06: '8月6日', aug08: '8月8日', aug10: '8月10日', today: '今日', systemHealth: 'システム状態', liveStatus: 'コンポーネントの現在状態',
    allOperational: 'すべて稼働中', connections: '12接続', failClosed: 'フェイルクローズ', attention: '要確認', onlineCount: '2台オンライン', secretStore: 'シークレットストア',
    viewSystemDetails: 'システム詳細を表示', recentRouting: '最近のルーティング判断', selectionReason: '各推論エンジンが選択された理由',
    viewRouting: 'ルーティングを表示', time: '時刻', application: 'アプリケーション', task: 'タスク', selectedModel: '選択モデル',
    policy: 'ポリシー', latency: '遅延', status: '状態', complete: '完了', models: 'モデル', manage: '管理', connect: '接続',
    notConfigured: '未設定', connected: '接続済み', searchModels: '登録済みモデルを検索', capabilities: '機能', model: 'モデル', provider: 'プロバイダー',
    execution: '実行場所', context: 'コンテキスト', health: '状態', cloud: 'クラウド', local: 'ローカル', totalRecords: '総レコード数',
    hitRate: '検索ヒット率', localOnlyRecords: 'ローカル限定レコード', searchMemory: '現在のスコープ内のメモリを検索', scopeType: 'スコープと種類',
    updated: '更新', project: 'プロジェクト', decision: '決定', knowledge: '知識', experience: '経験', high: '高', critical: '最重要', medium: '中',
    diagnosticRun: '診断実行', manual: '手動', testPipeline: 'インテリジェンスパイプラインをテスト',
    testPipelineDescription: 'プロバイダー応答、ルーティング、検索されたメモリ、最終的なVertex Contextを確認します。このコンソールは診断専用で、利用者向けチャットではありません。',
    diagnosticPrompt: '診断プロンプト', enterTask: '診断タスクを入力…', contextInspection: 'コンテキスト検査が有効', runTest: 'テスト実行',
    runInspector: '実行インスペクター', contextRoutingDetails: 'コンテキストとルーティングの詳細',
    runInspectorEmpty: 'テストを実行すると、選択モデル、プライバシーチェック、トークン予算、メモリ検索を確認できます。',
    managementBoundary: '管理境界', placeholderSuffix: 'この画面はUIアーキテクチャに含まれており、連携フェーズでトランスポート非依存のCoreコマンドへ接続します。',
    openConfiguration: '設定を開く', integrationReadiness: '連携準備状況', uiBoundaryDefined: 'UI境界を定義済み', coreHeadless: 'Coreはヘッドレスを維持', noDirectDatabase: 'データベースへ直接アクセスしない',
    providerConnection: 'プロバイダー接続', connectProvider: 'に接続', providerSecretCopy: 'キーはオペレーティングシステムの資格情報ストアへ送信されます。Vertex AIはAPIキーをPostgreSQLやログへ保存しません。',
    apiKey: 'APIキー', enterApiKey: 'プロバイダーのAPIキーを入力', protectedByOs: 'OS資格情報で保護', plaintextDisabled: '平文へのフォールバックは無効です。',
    cancel: 'キャンセル', connectDiscover: '接続してモデルを検出', jumpPage: '画面へ移動…', navigation: 'ナビゲーション', go: '移動',
    apiKeyRequired: '続行するにはAPIキーを入力してください', validationQueued: '接続の検証を開始しました',
    language: '言語', languageTitle: '表示言語', languageDescription: '管理UIで使用する言語を選択します。製品名、モデル名、API名などの固有名詞は原文を維持します。',
    japanese: '日本語', japaneseDetail: '標準の表示言語', english: 'English', englishDetail: '英語表示', savedOnDevice: 'この端末に設定を保存します。',
    languageChanged: '表示言語を日本語に変更しました', settingsScope: '設定の適用範囲', settingsScopeDescription: '現在は管理UIの表示言語に適用されます。Coreの応答言語やモデルプロンプトには影響しません。',
    scanningEnvironment: '端末環境を確認しています…', desktopRequired: 'デスクトップ版で利用できます', desktopRequiredDescription: '環境の検出は安全なRust Coreを通じて行います。ブラウザプレビューからOS情報へはアクセスしません。',
    openDesktopHint: 'Tauri版を起動すると、このコンピューターで確認できた事実がここに表示されます。', scanFailed: '環境情報を取得できませんでした', retry: '再試行', technicalDetails: '技術情報',
    detectedAssets: '検出した項目', providedCapabilities: '確認できた機能', scannedRoots: '確認した検索場所', lastObserved: '最終確認', searchEnvironment: 'ツール、ランタイム、機能を検索',
    noEnvironmentAssets: '対象の項目は見つかりませんでした', verifiedByCore: 'Coreで確認済み', location: '場所', capability: '機能', category: '分類', assetKind: '種類',
    categoryAi: 'AI', categoryDeveloper: '開発', categoryCreator: '制作', categoryRuntime: 'ランタイム', categoryDatabase: 'データベース', categoryServer: 'サーバー', categorySystem: 'システム', categoryHardware: 'ハードウェア', categoryStorage: 'ストレージ',
    kindApplication: 'アプリケーション', kindExecutable: '実行ファイル', kindRuntime: 'ランタイム', kindService: 'サービス', kindProcess: 'プロセス', kindDependency: '依存関係', kindSdk: 'SDK', kindDriver: 'ドライバー', kindStorageDevice: 'ストレージデバイス', kindConfiguration: '設定',
    environmentUpdated: '環境インデックスを更新しました', noEnvironmentChanges: '端末環境に変更はありません', addedShort: '追加', updatedShort: '更新', removedShort: '削除',
    localAiChecking: 'ローカルAIを確認しています…', localAiReady: 'Ollama 接続済み', localAiUnavailable: 'Ollamaに接続できません',
    localAiUnavailableHint: 'Ollamaを起動し、利用するモデルをインストールしてください。クラウドへ自動転送されることはありません。',
    noLocalModels: '利用可能なローカルモデルがありません', selectLocalModel: 'ローカルモデルを選択', response: '応答',
    localOnlyExecution: '端末内のみで実行', inputTokens: '入力トークン', outputTokens: '出力トークン', running: '実行中…',
    promptRequired: 'プロンプトとモデルを選択してください', desktopAiRequired: 'ローカルAI実行はデスクトップ版で利用できます。',
    runtimeOverview: 'ランタイム概要', runtimeReady: 'ローカル推論可能', runtimeOffline: 'オフライン', installedModels: '導入モデル', loadedModels: 'ロード中', modelStorage: 'モデル保存先', executable: '実行ファイル', endpoint: '接続先', version: 'バージョン', totalModelSize: 'モデル総容量', vramInUse: 'VRAM使用量', releaseMemory: 'メモリから解放', releasingMemory: '解放中…', memoryReleased: 'モデルをメモリから解放しました', noLoadedModels: '現在メモリにロードされたモデルはありません', checkedAt: '確認時刻', localAiFacts: '端末内AI資産の確認済み情報', environmentLoading: 'AI環境を確認しています…', environmentUnavailable: 'AI環境を取得できませんでした', modelDetails: 'モデル詳細', quantization: '量子化', family: 'ファミリー', digest: 'ダイジェスト', contextLength: 'コンテキスト長', expiresAt: '自動解放予定',
    downloadModel: 'ローカルモデルを追加', modelName: 'Ollamaモデル名', modelNameHint: '例: qwen3:4b', download: 'ダウンロード開始', downloading: '開始中…', downloadJobs: 'ダウンロード状況', noDownloadJobs: 'ダウンロード履歴はありません', cancelDownload: '中止', freeSpace: '空き容量', storageCapacity: '保存領域', storageUsed: '使用中', downloadStarted: 'モデルのダウンロードを開始しました', downloadSucceeded: '完了', downloadCancelled: '中止済み', downloadFailed: '失敗', downloadQueued: '待機中', downloadRunning: 'ダウンロード中', downloadCancelling: '中止処理中', downloadInterrupted: 'アプリ再起動により中断',
    operationalCenter: 'Vertex AI オペレーショナルセンター', operationalCenterDescription: '現在のAI環境と稼働状況', actionRequired: '対応が必要', actionCenterDescription: '現在実行できる安全な操作', systemHealthScore: 'システム健全性', activeModels: 'アクティブモデル', runningTasks: '実行中タスク', runtimeStatus: 'ランタイム状態', verifiedLive: '実データ', dataUnavailable: '未確認', apiNotConnected: '計測API接続待ち', inspectInDesktop: 'デスクトップ版で端末状態を確認できます。', noActionRequired: '緊急対応はありません', noActionDetail: '確認できたローカルAI環境に重大な問題はありません。', reviewEnvironment: '環境を確認', diskSpaceLow: 'モデル保存先の空き容量が少なくなっています', diskSpaceLowDetail: '安全なモデル移動・削除機能は未実装です。詳細画面で容量を確認してください。', modelMissing: 'ローカルモデルがありません', modelMissingDetail: 'モデル画面からOllamaモデルをバックグラウンドで追加できます。', ollamaUnavailable: 'Ollamaに接続できません', ollamaUnavailableDetail: 'Ollamaを起動してから状態を再確認してください。クラウドへは自動転送しません。', downloadFailure: 'モデル取得に失敗した履歴があります', downloadFailureDetail: 'モデル画面で失敗理由を確認し、必要に応じて再実行してください。', viewDetails: '詳細を表示', recommendations: 'Vertex推奨', recommendationHealthy: '現時点で安全に実行すべき改善操作はありません。', resourceMonitor: 'リソースモニター', resourceMonitorDescription: '端末リソースの現在値', telemetryPending: 'テレメトリー未接続', vramAllocated: 'VRAM割当', storageStatus: 'ストレージ', modelLocation: 'モデル保存先', aiActivity: 'AIアクティビティ', aiActivityDescription: '実行中のAI処理と将来のエージェント接続', noRunningAiTasks: '実行中のAIバックグラウンド処理はありません', futureAgentConnection: 'エージェント活動API接続待ち', activeDownloads: 'モデル取得', lastResponse: '最終応答', knowledgeCore: 'Knowledge Core', connectionPending: '接続確認API未実装', loadedModel: 'ロード済みモデル', diskFree: '空き', issues: '問題', currentState: '現在の状態', operational: '稼働中', warningStatus: '警告', offlineStatus: 'オフライン', notMeasured: '未計測', providerReady: '利用可能',
    memoryCore: 'Vertex Memory Core', memoryCoreReady: '利用可能', memoryCoreUnavailable: '現在利用できません', memoryCoreStarting: '初期化・起動中…', memoryCoreDetails: '技術詳細', databaseSize: 'データベース容量', activeConnections: '接続数', schemaVersion: 'スキーマ', start: '起動', stop: '停止', restart: '再起動', saveMemory: '記憶を保存', memoryContent: '記憶する内容を入力', noMemoryRecords: 'システムメモリはまだありません', diagnose: '診断', diagnosisClear: '問題は検出されませんでした', managedRuntime: 'Vertex管理Runtime',
  },
  en: {
    nav: {
      Dashboard: 'Dashboard', Models: 'Models', Providers: 'Providers', Memory: 'Memory', Routing: 'Routing', Applications: 'Applications',
      'AI Environment': 'AI Environment Manager', 'Local Runtimes': 'Local Runtimes', 'Environment Explorer': 'Environment Explorer', Storage: 'Storage', 'Edge Cores': 'Edge Cores', Security: 'Security', 'System Status': 'System Status', Logs: 'Logs', 'AI Test Console': 'AI Test Console', 'Developer Agent': 'Developer Agent', Settings: 'Settings',
    },
    groups: { intelligence: 'Intelligence', runtime: 'Runtime', operations: 'Operations', developer: 'Developer' },
    descriptions: {
      Dashboard: 'System overview and intelligence activity', Models: 'Registered reasoning engines and capabilities', Providers: 'Provider connections and model discovery',
      Memory: 'Persistent experience, scope, and retrieval', Routing: 'Model selection policies and routing decisions', Applications: 'Products connected to Vertex AI Core', 'AI Environment': 'Manage local AI runtimes, models, and resource usage', 'Local Runtimes': 'Inspect local inference engines and loaded model memory', 'Environment Explorer': 'Discover tools, runtimes, and capabilities on this computer', Storage: 'Manage models, caches, locations, and available space',
      'Edge Cores': 'Local autonomy and offline operation', Security: 'Secrets, permissions, and privacy boundaries', 'System Status': 'Core services, health, and performance',
      Logs: 'Operational events without private content', 'AI Test Console': 'Developer diagnostics for providers and context', 'Developer Agent': 'Autonomous repository analysis, editing, and validation within a safe workspace', Settings: 'Core behavior and environment configuration',
    },
    closeNavigation: 'Close navigation', primaryNavigation: 'Primary navigation', openNavigation: 'Open navigation', coreOnline: 'Core online', localVersion: 'v0.1.6 · Local',
    administrator: 'Administrator', localWorkspace: 'Local workspace', searchGlobal: 'Search models, memory, settings…', previewData: 'Preview data', notifications: 'Notifications',
    localCore: 'Local Core', management: 'Vertex AI Management', refresh: 'Refresh', addProvider: 'Add provider', thisWeek: '+2 this week', availableModels: 'Available models',
    connectedProviders: '2 providers connected', memoryRecords: 'Memory records', retrievalPrecision: '98.7% retrieval precision', last24h: 'Last 24h', aiOperations: 'AI operations',
    servedLocally: '64% served locally', healthy: 'Healthy', degraded: 'Degraded', coreAvailability: 'Core availability', commandLatency: '28 ms command latency',
    intelligenceActivity: 'Intelligence activity', routedByLocation: 'Requests routed by execution location', last7Days: 'Last 7 days', localModels: 'Local models', cloudModels: 'Cloud models',
    activityChart: 'Local and cloud activity chart', aug06: 'Aug 06', aug08: 'Aug 08', aug10: 'Aug 10', today: 'Today', systemHealth: 'System health', liveStatus: 'Live component status',
    allOperational: 'All operational', connections: '12 connections', failClosed: 'Fail-closed', attention: 'Attention', onlineCount: '2 online', secretStore: 'Secret Store', viewSystemDetails: 'View system details',
    recentRouting: 'Recent routing decisions', selectionReason: 'Why each reasoning engine was selected', viewRouting: 'View routing', time: 'Time', application: 'Application', task: 'Task',
    selectedModel: 'Selected model', policy: 'Policy', latency: 'Latency', status: 'Status', complete: 'Complete', models: 'Models', manage: 'Manage', connect: 'Connect',
    notConfigured: 'Not configured', connected: 'Connected', searchModels: 'Search registered models', capabilities: 'Capabilities', model: 'Model', provider: 'Provider', execution: 'Execution',
    context: 'Context', health: 'Health', cloud: 'Cloud', local: 'Local', totalRecords: 'Total records', hitRate: 'Retrieval hit rate', localOnlyRecords: 'Local-only records',
    searchMemory: 'Search memory in current scope', scopeType: 'Scope & type', updated: 'Updated', project: 'Project', decision: 'Decision', knowledge: 'Knowledge', experience: 'Experience',
    high: 'High', critical: 'Critical', medium: 'Medium', diagnosticRun: 'Diagnostic run', manual: 'Manual', testPipeline: 'Test the intelligence pipeline',
    testPipelineDescription: 'Inspect provider responses, routing, retrieved Memory, and the final Vertex Context. This console is for diagnostics—not end-user chat.', diagnosticPrompt: 'Diagnostic prompt',
    enterTask: 'Enter a diagnostic task…', contextInspection: 'Context inspection enabled', runTest: 'Run test', runInspector: 'Run inspector', contextRoutingDetails: 'Context and routing details',
    runInspectorEmpty: 'Run a test to inspect the selected model, privacy checks, token budget, and Memory retrieval.', managementBoundary: 'Management boundary',
    placeholderSuffix: 'This screen is included in the UI architecture and will connect to transport-neutral Core commands in the integration phase.', openConfiguration: 'Open configuration',
    integrationReadiness: 'Integration readiness', uiBoundaryDefined: 'UI boundary defined', coreHeadless: 'Core remains headless', noDirectDatabase: 'No direct database access',
    providerConnection: 'Provider connection', connectProvider: 'Connect ', providerSecretCopy: 'Your key will be sent to the operating-system Secret Store. Vertex AI never stores API keys in PostgreSQL or logs.',
    apiKey: 'API key', enterApiKey: 'Enter provider API key', protectedByOs: 'Protected by OS credentials', plaintextDisabled: 'Plaintext fallback is disabled.', cancel: 'Cancel',
    connectDiscover: 'Connect & discover models', jumpPage: 'Jump to a page…', navigation: 'Navigation', go: 'Go', apiKeyRequired: 'Enter an API key to continue', validationQueued: ' connection queued for validation',
    language: 'Language', languageTitle: 'Display language', languageDescription: 'Choose the language used by the management UI. Product, model, and API names remain unchanged.',
    japanese: '日本語', japaneseDetail: 'Japanese interface', english: 'English', englishDetail: 'Default English interface', savedOnDevice: 'This preference is saved on this device.',
    languageChanged: 'Display language changed to English', settingsScope: 'Setting scope', settingsScopeDescription: 'This currently applies to the management UI only. It does not change Core response language or model prompts.',
    scanningEnvironment: 'Inspecting this computer…', desktopRequired: 'Available in the desktop application', desktopRequiredDescription: 'Environment discovery runs through the safe Rust Core. The browser preview does not access operating-system information.',
    openDesktopHint: 'Start the Tauri application to display verified facts from this computer here.', scanFailed: 'Could not retrieve environment information', retry: 'Retry', technicalDetails: 'Technical details',
    detectedAssets: 'Detected assets', providedCapabilities: 'Verified capabilities', scannedRoots: 'Search locations checked', lastObserved: 'Last observed', searchEnvironment: 'Search tools, runtimes, or capabilities',
    noEnvironmentAssets: 'No matching assets were found', verifiedByCore: 'Verified by Core', location: 'Location', capability: 'Capabilities', category: 'Category', assetKind: 'Kind',
    categoryAi: 'AI', categoryDeveloper: 'Developer', categoryCreator: 'Creator', categoryRuntime: 'Runtime', categoryDatabase: 'Database', categoryServer: 'Server', categorySystem: 'System', categoryHardware: 'Hardware', categoryStorage: 'Storage',
    kindApplication: 'Application', kindExecutable: 'Executable', kindRuntime: 'Runtime', kindService: 'Service', kindProcess: 'Process', kindDependency: 'Dependency', kindSdk: 'SDK', kindDriver: 'Driver', kindStorageDevice: 'Storage device', kindConfiguration: 'Configuration',
    environmentUpdated: 'Environment index updated', noEnvironmentChanges: 'No environment changes detected', addedShort: 'added', updatedShort: 'updated', removedShort: 'removed',
    localAiChecking: 'Checking local AI…', localAiReady: 'Ollama connected', localAiUnavailable: 'Cannot connect to Ollama',
    localAiUnavailableHint: 'Start Ollama and install a model. Vertex AI will never fall back to cloud automatically.',
    noLocalModels: 'No local models are available', selectLocalModel: 'Select a local model', response: 'Response',
    localOnlyExecution: 'Local-only execution', inputTokens: 'Input tokens', outputTokens: 'Output tokens', running: 'Running…',
    promptRequired: 'Select a model and enter a prompt', desktopAiRequired: 'Local AI execution is available in the desktop application.',
    runtimeOverview: 'Runtime overview', runtimeReady: 'Local inference ready', runtimeOffline: 'Offline', installedModels: 'Installed models', loadedModels: 'Loaded models', modelStorage: 'Model storage', executable: 'Executable', endpoint: 'Endpoint', version: 'Version', totalModelSize: 'Total model size', vramInUse: 'VRAM in use', releaseMemory: 'Unload from memory', releasingMemory: 'Unloading…', memoryReleased: 'Model unloaded from memory', noLoadedModels: 'No models are currently loaded in memory', checkedAt: 'Checked at', localAiFacts: 'Verified local AI assets', environmentLoading: 'Inspecting AI environment…', environmentUnavailable: 'Could not inspect AI environment', modelDetails: 'Model details', quantization: 'Quantization', family: 'Family', digest: 'Digest', contextLength: 'Context length', expiresAt: 'Scheduled unload',
    downloadModel: 'Add a local model', modelName: 'Ollama model name', modelNameHint: 'Example: qwen3:4b', download: 'Start download', downloading: 'Starting…', downloadJobs: 'Download activity', noDownloadJobs: 'No download history', cancelDownload: 'Cancel', freeSpace: 'Free space', storageCapacity: 'Storage capacity', storageUsed: 'Used', downloadStarted: 'Model download started', downloadSucceeded: 'Completed', downloadCancelled: 'Cancelled', downloadFailed: 'Failed', downloadQueued: 'Queued', downloadRunning: 'Downloading', downloadCancelling: 'Cancelling', downloadInterrupted: 'Interrupted by application restart',
    operationalCenter: 'Vertex AI Operational Center', operationalCenterDescription: 'Current AI environment and operations', actionRequired: 'Action required', actionCenterDescription: 'Safe actions currently available', systemHealthScore: 'System health', activeModels: 'Active models', runningTasks: 'Running tasks', runtimeStatus: 'Runtime status', verifiedLive: 'Live data', dataUnavailable: 'Unverified', apiNotConnected: 'Telemetry API pending', inspectInDesktop: 'Use the desktop application to inspect this computer.', noActionRequired: 'No urgent action required', noActionDetail: 'No critical issue was found in the verified local AI environment.', reviewEnvironment: 'Inspect environment', diskSpaceLow: 'Model storage is running low', diskSpaceLowDetail: 'Safe model migration and removal are not implemented yet. Inspect capacity in the environment view.', modelMissing: 'No local models installed', modelMissingDetail: 'Add an Ollama model in the Models view as a background job.', ollamaUnavailable: 'Cannot connect to Ollama', ollamaUnavailableDetail: 'Start Ollama and refresh. Vertex AI will not fall back to cloud automatically.', downloadFailure: 'A model download has failed', downloadFailureDetail: 'Inspect the failure in Models and retry when appropriate.', viewDetails: 'View details', recommendations: 'Vertex recommendation', recommendationHealthy: 'There is no verified improvement action that must be run now.', resourceMonitor: 'Resource monitor', resourceMonitorDescription: 'Current device resource values', telemetryPending: 'Telemetry not connected', vramAllocated: 'VRAM allocated', storageStatus: 'Storage', modelLocation: 'Model storage', aiActivity: 'AI activity', aiActivityDescription: 'Running AI work and future agent connections', noRunningAiTasks: 'No AI background task is running', futureAgentConnection: 'Agent activity API pending', activeDownloads: 'Model download', lastResponse: 'Last response', knowledgeCore: 'Knowledge Core', connectionPending: 'Health API not implemented', loadedModel: 'Loaded model', diskFree: 'free', issues: 'Issues', currentState: 'Current state', operational: 'Running', warningStatus: 'Warning', offlineStatus: 'Offline', notMeasured: 'Not measured', providerReady: 'Available',
    memoryCore: 'Vertex Memory Core', memoryCoreReady: 'Available', memoryCoreUnavailable: 'Currently unavailable', memoryCoreStarting: 'Initializing…', memoryCoreDetails: 'Technical details', databaseSize: 'Database size', activeConnections: 'Connections', schemaVersion: 'Schema', start: 'Start', stop: 'Stop', restart: 'Restart', saveMemory: 'Save memory', memoryContent: 'Enter something to remember', noMemoryRecords: 'No system memories yet', diagnose: 'Diagnose', diagnosisClear: 'No problem detected', managedRuntime: 'Vertex-managed runtime',
  },
} as const

const navItems: NavItem[] = [
  { key: 'Dashboard', icon: LayoutDashboard },
  { key: 'Models', icon: BrainCircuit, group: 'intelligence' },
  { key: 'Providers', icon: Blocks },
  { key: 'Memory', icon: Database },
  { key: 'Routing', icon: Route },
  { key: 'Applications', icon: AppWindow, group: 'runtime' },
  { key: 'AI Environment', icon: Cpu },
  { key: 'Local Runtimes', icon: Power },
  { key: 'Environment Explorer', icon: Search },
  { key: 'Storage', icon: Database },
  { key: 'Edge Cores', icon: Network },
  { key: 'Security', icon: ShieldCheck },
  { key: 'System Status', icon: Activity, group: 'operations' },
  { key: 'Logs', icon: TerminalSquare },
  { key: 'AI Test Console', icon: FlaskConical, group: 'developer' },
  { key: 'Developer Agent', icon: Code2 },
  { key: 'Settings', icon: Settings },
]

const savedLocale = window.localStorage.getItem('vertex-ai-locale')
const locale = ref<Locale>(savedLocale === 'en' ? 'en' : 'ja')
const copy = computed(() => messages[locale.value])
const activePage = ref<PageKey>('Dashboard')
const sidebarOpen = ref(false)
const providerDialog = ref(false)
const commandOpen = ref(false)
const selectedProvider = ref('OpenAI')
const apiKey = ref('')
const toast = ref('')
type EnvironmentUiState =
  | { kind: 'idle' | 'loading' | 'desktop_required' }
  | { kind: 'ready'; snapshot: EnvironmentSnapshot }
  | { kind: 'error'; message: string; technical: string }
const environmentState = ref<EnvironmentUiState>({ kind: 'idle' })
const environmentSearch = ref('')
const environmentRefreshing = ref(false)
type LocalAiUiState =
  | { kind: 'idle' }
  | { kind: 'loading' }
  | { kind: 'desktop_required' }
  | { kind: 'ready'; status: LocalProviderStatus }
  | { kind: 'error'; message: string; technical: string }
const localAiState = ref<LocalAiUiState>({ kind: 'idle' })
const selectedLocalModel = ref('')
const localPrompt = ref('')
const localRunPending = ref(false)
const localResponse = ref<GenerationResponse | null>(null)
const localRunError = ref('')
type AiEnvironmentUiState =
  | { kind: 'idle' }
  | { kind: 'loading' }
  | { kind: 'desktop_required' }
  | { kind: 'ready'; summary: AiEnvironmentSummary }
  | { kind: 'error'; message: string }
const aiEnvironmentState = ref<AiEnvironmentUiState>({ kind: 'idle' })
const unloadingModel = ref('')
const downloadModelName = ref('')
const downloadJobs = ref<ModelDownloadJob[]>([])
const downloadBusy = ref(false)
type ModelManagementUiState =
  | { kind: 'idle' | 'loading' | 'desktop_required' }
  | { kind: 'ready'; snapshot: ModelManagementSnapshot }
  | { kind: 'error'; message: string }
const modelManagementState = ref<ModelManagementUiState>({ kind: 'idle' })
const modelStorageBusy = ref(false)
const modelManagerError = ref('')
type MemoryUiState =
  | { kind: 'idle' }
  | { kind: 'loading' }
  | { kind: 'desktop_required' }
  | { kind: 'ready'; status: MemoryCoreStatus; records: MemoryRecord[] }
  | { kind: 'error'; message: string }
const memoryState = ref<MemoryUiState>({ kind: 'idle' })
const memorySearch = ref('')
const newMemoryContent = ref('')
const memoryBusy = ref(false)
const memoryDiagnoses = ref<RuntimeDiagnosis[]>([])
const developerWorkspaces = ref<DeveloperWorkspace[]>([])
const developerModes: DeveloperMode[] = ['ASK', 'READ_ONLY', 'EDIT', 'EXECUTE', 'AUTO']
const selectedDeveloperWorkspace = ref('')
const developerWorkspaceName = ref('Vertex AI')
const developerWorkspaceRoot = ref('')
const developerMode = ref<DeveloperMode>('READ_ONLY')
const developerRequest = ref('現在のRuntime Managerの構造を調査し、コード変更は行わず概要を報告してください。')
const developerTask = ref<DeveloperTask | null>(null)
const developerBusy = ref(false)
const developerError = ref('')
const ardTeams = ref<ArdTeam[]>([])
const ardWorkflows = ref<ArdWorkflow[]>([])
const selectedArdTeam = ref('')
const selectedArdWorkflow = ref('')
const ardSession = ref<ArdSession | null>(null)
const ardTeamName = ref('Vertex Development Team')
const ardGoal = ref('既存設計を尊重し、安全に実装・レビュー・テストしてください。')
let downloadPollTimer: number | undefined
let developerPollTimer: number | undefined
let ardPollTimer: number | undefined

const providers = computed(() => [
  {
    name: 'Ollama',
    detail: locale.value === 'ja' ? 'ローカル限定 · 127.0.0.1:11434' : 'Loopback only · 127.0.0.1:11434',
    models: localAiState.value.kind === 'ready' ? localAiState.value.status.models.length : 0,
    configured: localAiState.value.kind === 'ready' && localAiState.value.status.health.state === 'healthy',
    latency: localAiState.value.kind === 'loading' ? '…' : '—',
    tone: 'cyan',
  },
  { name: 'OpenAI', detail: 'Responses API', models: 0, configured: false, latency: '—', tone: 'violet' },
])

const models = computed(() => modelManagementState.value.kind === 'ready'
  ? modelManagementState.value.snapshot.models
  : [])
const modelCompatibility = computed(() => new Map(
  modelManagementState.value.kind === 'ready'
    ? modelManagementState.value.snapshot.compatibility.map((item) => [item.model_id, item])
    : [],
))

const routes = computed(() => locale.value === 'ja' ? [
  { time: '14:42:18', app: 'Vertex Studio', task: 'コード推論', model: 'gpt-5.6-terra', mode: '自動', latency: '1.8秒' },
  { time: '14:41:52', app: 'Stable Master', task: '知識検索', model: 'qwen3:14b', mode: 'ローカル', latency: '640 ms' },
  { time: '14:40:07', app: 'Vertex Studio', task: 'コンテキスト構築', model: 'gpt-5.6-luna', mode: '手動', latency: '924 ms' },
  { time: '14:38:24', app: 'Medical Record', task: '記録の要約', model: 'qwen3:14b', mode: 'プライバシー', latency: '1.2秒' },
] : [
  { time: '14:42:18', app: 'Vertex Studio', task: 'Code reasoning', model: 'gpt-5.6-terra', mode: 'Auto', latency: '1.8 s' },
  { time: '14:41:52', app: 'Stable Master', task: 'Knowledge lookup', model: 'qwen3:14b', mode: 'Local', latency: '640 ms' },
  { time: '14:40:07', app: 'Vertex Studio', task: 'Context build', model: 'gpt-5.6-luna', mode: 'Manual', latency: '924 ms' },
  { time: '14:38:24', app: 'Medical Record', task: 'Record summary', model: 'qwen3:14b', mode: 'Privacy', latency: '1.2 s' },
])

const memoryRows = computed(() => memoryState.value.kind === 'ready'
  ? memoryState.value.records.map((memory) => ({
      id: memory.memory_id,
      type: memoryCategoryLabel(memory.category),
      scope: locale.value === 'ja' ? 'システム' : 'System',
      excerpt: memory.content,
      priority: memory.priority >= 0.8 ? copy.value.high : copy.value.medium,
      priorityClass: memory.priority >= 0.9 ? 'critical' : memory.priority >= 0.7 ? 'high' : 'medium',
      updated: observedLabel(memory.updated_at),
    }))
  : [])

const activePageLabel = computed(() => copy.value.nav[activePage.value])
const pageDescription = computed(() => copy.value.descriptions[activePage.value])
const filteredEnvironmentAssets = computed(() => {
  if (environmentState.value.kind !== 'ready') return []
  const query = environmentSearch.value.trim().toLowerCase()
  if (!query) return environmentState.value.snapshot.assets
  return environmentState.value.snapshot.assets.filter((asset) =>
    asset.name.toLowerCase().includes(query)
    || asset.location?.toLowerCase().includes(query)
    || asset.capabilities.some((capability) => capability.toLowerCase().includes(query)),
  )
})
const environmentCapabilityCount = computed(() => {
  if (environmentState.value.kind !== 'ready') return 0
  return new Set(environmentState.value.snapshot.assets.flatMap((asset) => asset.capabilities)).size
})
const localModels = computed(() => localAiState.value.kind === 'ready' ? localAiState.value.status.models : [])
const latestDeveloperPlan = computed(() => developerTask.value?.plan_revisions.at(-1) ?? null)
const developerTaskActive = computed(() => developerTask.value
  ? !['COMPLETED', 'FAILED', 'CANCELLED', 'WAITING_APPROVAL'].includes(developerTask.value.state)
  : false)
const developerValidation = computed(() => {
  const commands = developerTask.value?.commands ?? []
  if (!commands.length) return '—'
  return commands.every((command) => command.status === 'COMPLETED') ? 'PASS' : 'FAIL'
})
const dashboardRuntime = computed(() => aiEnvironmentState.value.kind === 'ready' ? aiEnvironmentState.value.summary.runtimes[0] ?? null : null)
const activeDownloads = computed(() => downloadJobs.value.filter((job) => ['queued', 'running', 'cancelling'].includes(job.state)))
const failedDownloads = computed(() => downloadJobs.value.filter((job) => job.state === 'failed'))
const dashboardLoadedModels = computed(() => aiEnvironmentState.value.kind === 'ready' ? aiEnvironmentState.value.summary.loaded_model_count : null)
const dashboardInstalledModels = computed(() => aiEnvironmentState.value.kind === 'ready' ? aiEnvironmentState.value.summary.installed_model_count : null)
const dashboardStoragePercent = computed(() => storageUsedPercent(dashboardRuntime.value?.storage_total_bytes ?? null, dashboardRuntime.value?.storage_available_bytes ?? null))
const dashboardIssueCount = computed(() => {
  if (aiEnvironmentState.value.kind !== 'ready') return null
  let count = 0
  if (aiEnvironmentState.value.summary.ready_runtime_count === 0) count += 1
  if (aiEnvironmentState.value.summary.installed_model_count === 0) count += 1
  if (dashboardStoragePercent.value >= 90) count += 1
  if (failedDownloads.value.length > 0) count += 1
  return count
})
const dashboardHealthScore = computed(() => {
  if (dashboardIssueCount.value == null) return null
  return Math.max(0, 100 - dashboardIssueCount.value * 14)
})
const dashboardAction = computed(() => {
  if (aiEnvironmentState.value.kind === 'desktop_required') return { tone: 'neutral', title: copy.value.desktopRequired, detail: copy.value.inspectInDesktop, page: 'AI Environment' as PageKey }
  if (aiEnvironmentState.value.kind !== 'ready') return { tone: 'neutral', title: copy.value.environmentLoading, detail: copy.value.currentState, page: 'AI Environment' as PageKey }
  if (aiEnvironmentState.value.summary.ready_runtime_count === 0) return { tone: 'danger', title: copy.value.ollamaUnavailable, detail: copy.value.ollamaUnavailableDetail, page: 'Local Runtimes' as PageKey }
  if (dashboardStoragePercent.value >= 90) return { tone: 'warning', title: copy.value.diskSpaceLow, detail: copy.value.diskSpaceLowDetail, page: 'AI Environment' as PageKey }
  if (aiEnvironmentState.value.summary.installed_model_count === 0) return { tone: 'warning', title: copy.value.modelMissing, detail: copy.value.modelMissingDetail, page: 'Models' as PageKey }
  if (failedDownloads.value.length > 0) return { tone: 'warning', title: copy.value.downloadFailure, detail: copy.value.downloadFailureDetail, page: 'Models' as PageKey }
  return { tone: 'healthy', title: copy.value.noActionRequired, detail: copy.value.noActionDetail, page: 'AI Environment' as PageKey }
})

watch(locale, (next) => {
  window.localStorage.setItem('vertex-ai-locale', next)
  document.documentElement.lang = next
  document.title = next === 'ja' ? 'Vertex AI 管理コンソール' : 'Vertex AI Management'
}, { immediate: true })
watch(activePage, (page) => {
  if (page === 'Environment Explorer' && environmentState.value.kind === 'idle') void loadEnvironment()
  if (['Dashboard', 'AI Test Console', 'Models', 'Providers'].includes(page) && localAiState.value.kind === 'idle') void loadLocalAi()
  if (page === 'Models' && modelManagementState.value.kind === 'idle') void loadModelManagement()
  if (['Dashboard', 'AI Environment', 'Local Runtimes'].includes(page) && aiEnvironmentState.value.kind === 'idle') void loadAiEnvironment()
  if (['Dashboard', 'Models'].includes(page)) void loadDownloadJobs()
  if (['Dashboard', 'Memory', 'AI Environment', 'Local Runtimes', 'System Status'].includes(page) && memoryState.value.kind === 'idle') void loadMemoryCore()
  if (page === 'Developer Agent') void loadDeveloperAgent()
}, { immediate: true })
onUnmounted(() => {
  window.clearTimeout(downloadPollTimer)
  window.clearTimeout(developerPollTimer)
  window.clearTimeout(ardPollTimer)
})

function navLabel(key: PageKey) { return copy.value.nav[key] }
function groupLabel(key?: GroupKey) { return key ? copy.value.groups[key] : '' }
function navigate(key: PageKey) {
  activePage.value = key
  sidebarOpen.value = false
  commandOpen.value = false
}
async function loadEnvironment(force = false) {
  if (!force && (environmentState.value.kind === 'loading' || environmentState.value.kind === 'ready')) return
  if (!force && environmentState.value.kind === 'idle') {
    try {
      const cached = await getCachedEnvironment()
      if (cached) environmentState.value = { kind: 'ready', snapshot: cached }
    } catch {
      // A cache miss or unreadable cache must not block a fresh deterministic scan.
    }
  }
  const hadSnapshot = environmentState.value.kind === 'ready'
  if (!hadSnapshot) environmentState.value = { kind: 'loading' }
  environmentRefreshing.value = true
  const result = await scanEnvironment()
  environmentRefreshing.value = false
  if (result.kind === 'ready') {
    environmentState.value = { kind: 'ready', snapshot: result.result.snapshot }
    const delta = result.result.delta
    if (delta) {
      const total = delta.added.length + delta.updated.length + delta.removed.length
      toast.value = total === 0 && !delta.relationships_changed
        ? copy.value.noEnvironmentChanges
        : `${copy.value.environmentUpdated}（${copy.value.addedShort} ${delta.added.length}・${copy.value.updatedShort} ${delta.updated.length}・${copy.value.removedShort} ${delta.removed.length}）`
      window.setTimeout(() => (toast.value = ''), 3200)
    }
  }
  else if (result.kind === 'desktop_required') environmentState.value = { kind: 'desktop_required' }
  else if (!hadSnapshot) environmentState.value = { kind: 'error', message: result.error.human_fallback_message, technical: `${result.error.machine_readable_code}: ${result.error.technical_message}` }
  else {
    toast.value = result.error.human_fallback_message
    window.setTimeout(() => (toast.value = ''), 3200)
  }
}
function refreshPage() {
  if (activePage.value === 'Environment Explorer') void loadEnvironment(true)
  if (['AI Test Console', 'Models', 'Providers'].includes(activePage.value)) void loadLocalAi()
  if (['AI Environment', 'Local Runtimes'].includes(activePage.value)) void loadAiEnvironment()
  if (activePage.value === 'Models') void loadDownloadJobs()
  if (activePage.value === 'Developer Agent') void loadDeveloperAgent()
  if (['Memory', 'AI Environment', 'Local Runtimes', 'System Status'].includes(activePage.value)) void loadMemoryCore()
  if (activePage.value === 'Dashboard') {
    void loadLocalAi()
    void loadAiEnvironment()
    void loadDownloadJobs()
    void loadMemoryCore()
  }
}

async function loadDeveloperAgent() {
  if (!isDesktopRuntime()) {
    developerError.value = 'desktop_required'
    return
  }
  developerError.value = ''
  try {
    developerWorkspaces.value = await listDeveloperWorkspaces()
    if (!developerWorkspaces.value.some((workspace) => workspace.id === selectedDeveloperWorkspace.value)) {
      selectedDeveloperWorkspace.value = developerWorkspaces.value[0]?.id ?? ''
    }
    ardTeams.value = await listArdTeams()
    if (!ardTeams.value.some((team) => team.id === selectedArdTeam.value)) {
      selectedArdTeam.value = ardTeams.value[0]?.id ?? ''
    }
    await loadArdWorkflows()
    const sessions = await listArdSessions(20)
    ardSession.value = sessions[0] ?? null
    if (localAiState.value.kind === 'idle') await loadLocalAi()
  } catch (error) {
    developerError.value = error instanceof Error ? error.message : String(error)
  }
}

async function loadArdWorkflows() {
  if (!selectedArdTeam.value) {
    ardWorkflows.value = []
    selectedArdWorkflow.value = ''
    return
  }
  ardWorkflows.value = await listArdWorkflows(selectedArdTeam.value)
  if (!ardWorkflows.value.some((workflow) => workflow.id === selectedArdWorkflow.value)) {
    selectedArdWorkflow.value = ardWorkflows.value[0]?.id ?? ''
  }
}

async function createArdPresetTeam() {
  if (!selectedDeveloperWorkspace.value || !ardTeamName.value.trim()) return
  developerBusy.value = true
  developerError.value = ''
  const brain = { kind: 'auto' as const }
  try {
    const team = await createArdTeam({
      name: ardTeamName.value.trim(),
      workspace_id: selectedDeveloperWorkspace.value,
      members: [
        { name: 'Alice', role: 'Architect', brain, permission: { allowed: ['read_files', 'git_read'], maximum_risk: 'LOW' }, responsibilities: ['設計調査と実装計画'], forbidden_actions: ['承認なしの設計変更', 'Workspace外操作'] },
        { name: 'Bob', role: 'Developer', brain, permission: { allowed: ['read_files', 'write_files', 'terminal', 'git_read'], maximum_risk: 'MEDIUM' }, responsibilities: ['最小差分の実装とBuild/Test'], forbidden_actions: ['破壊的コマンド', 'Workspace外操作'] },
        { name: 'Carol', role: 'Reviewer', brain, permission: { allowed: ['read_files', 'git_read'], maximum_risk: 'LOW' }, responsibilities: ['差分とテスト結果のレビュー'], forbidden_actions: ['ファイル変更', 'Git書込み'] },
      ],
    })
    const workflow = await createArdWorkflow(team.id, 'Architect → Developer → Reviewer')
    await loadDeveloperAgent()
    selectedArdTeam.value = team.id
    await loadArdWorkflows()
    selectedArdWorkflow.value = workflow.id
  } catch (error) {
    developerError.value = error instanceof Error ? error.message : String(error)
  } finally {
    developerBusy.value = false
  }
}

function latestBrainResolution(memberId: string) {
  return [...(ardSession.value?.brain_resolutions ?? [])]
    .reverse()
    .find((resolution) => resolution.member_id === memberId)
}

async function runArdRelay() {
  if (!selectedArdWorkflow.value || !ardGoal.value.trim()) return
  developerBusy.value = true
  developerError.value = ''
  try {
    ardSession.value = await startArdSession(selectedArdWorkflow.value, ardGoal.value.trim())
    scheduleArdPoll()
  } catch (error) {
    developerError.value = error instanceof Error ? error.message : String(error)
  } finally {
    developerBusy.value = false
  }
}

async function toggleArdPause() {
  if (!ardSession.value) return
  developerBusy.value = true
  try {
    ardSession.value = ardSession.value.state === 'PAUSED'
      ? await resumeArdSession(ardSession.value.id)
      : await pauseArdSession(ardSession.value.id)
    scheduleArdPoll()
  } catch (error) {
    developerError.value = error instanceof Error ? error.message : String(error)
  } finally {
    developerBusy.value = false
  }
}

async function stopArdRelay() {
  if (!ardSession.value) return
  developerBusy.value = true
  try {
    ardSession.value = await cancelArdSession(ardSession.value.id)
    window.clearTimeout(ardPollTimer)
  } catch (error) {
    developerError.value = error instanceof Error ? error.message : String(error)
  } finally {
    developerBusy.value = false
  }
}

function scheduleArdPoll() {
  window.clearTimeout(ardPollTimer)
  if (!ardSession.value || !['RUNNING', 'PAUSED'].includes(ardSession.value.state)) return
  ardPollTimer = window.setTimeout(() => void pollArdSession(ardSession.value!.id), 750)
}

async function pollArdSession(sessionId: string) {
  try {
    ardSession.value = await getArdSession(sessionId)
    scheduleArdPoll()
  } catch (error) {
    developerError.value = error instanceof Error ? error.message : String(error)
  }
}

async function addDeveloperWorkspace() {
  if (!developerWorkspaceName.value.trim() || !developerWorkspaceRoot.value.trim()) return
  developerBusy.value = true
  developerError.value = ''
  try {
    const workspace = await registerDeveloperWorkspace(developerWorkspaceName.value.trim(), developerWorkspaceRoot.value.trim())
    await loadDeveloperAgent()
    selectedDeveloperWorkspace.value = workspace.id
    developerWorkspaceRoot.value = ''
  } catch (error) {
    developerError.value = error instanceof Error ? error.message : String(error)
  } finally {
    developerBusy.value = false
  }
}

function scheduleDeveloperPoll() {
  window.clearTimeout(developerPollTimer)
  if (!developerTaskActive.value || !developerTask.value) return
  developerPollTimer = window.setTimeout(() => void pollDeveloperTask(developerTask.value!.id), 750)
}

async function pollDeveloperTask(taskId: string) {
  try {
    developerTask.value = await getDeveloperTask(taskId)
    scheduleDeveloperPoll()
  } catch (error) {
    developerError.value = error instanceof Error ? error.message : String(error)
  }
}

async function runDeveloperAgent() {
  if (!selectedDeveloperWorkspace.value || !selectedLocalModel.value || !developerRequest.value.trim()) return
  developerBusy.value = true
  developerError.value = ''
  window.clearTimeout(developerPollTimer)
  try {
    developerTask.value = await startDeveloperTask({
      workspace_id: selectedDeveloperWorkspace.value,
      request: developerRequest.value.trim(),
      mode: developerMode.value,
      provider_id: 'ollama',
      model_id: selectedLocalModel.value,
    })
    scheduleDeveloperPoll()
  } catch (error) {
    developerError.value = error instanceof Error ? error.message : String(error)
  } finally {
    developerBusy.value = false
  }
}

async function stopDeveloperAgent() {
  if (!developerTask.value) return
  developerBusy.value = true
  try {
    await cancelDeveloperTask(developerTask.value.id)
    await pollDeveloperTask(developerTask.value.id)
  } catch (error) {
    developerError.value = error instanceof Error ? error.message : String(error)
  } finally {
    developerBusy.value = false
  }
}

async function rollbackDeveloperChanges() {
  if (!developerTask.value) return
  developerBusy.value = true
  try {
    developerTask.value = await rollbackDeveloperTask(developerTask.value.id)
  } catch (error) {
    developerError.value = error instanceof Error ? error.message : String(error)
  } finally {
    developerBusy.value = false
  }
}

async function loadMemoryCore() {
  if (!isDesktopRuntime()) {
    memoryState.value = { kind: 'desktop_required' }
    return
  }
  memoryState.value = { kind: 'loading' }
  try {
    const status = await getMemoryCoreStatus()
    const records = status.state === 'READY' ? await searchSystemMemories(memorySearch.value) : []
    memoryState.value = { kind: 'ready', status, records }
  } catch (error) {
    memoryState.value = { kind: 'error', message: error instanceof Error ? error.message : String(error) }
  }
}

async function runMemoryAction(action: 'start' | 'stop' | 'restart') {
  memoryBusy.value = true
  try {
    const status = action === 'start'
      ? await startMemoryCore()
      : action === 'stop'
        ? await stopMemoryCore()
        : await restartMemoryCore()
    const records = status.state === 'READY' ? await searchSystemMemories(memorySearch.value) : []
    memoryState.value = { kind: 'ready', status, records }
  } catch (error) {
    toast.value = error instanceof Error ? error.message : String(error)
    window.setTimeout(() => (toast.value = ''), 3200)
    await loadMemoryCore()
  } finally {
    memoryBusy.value = false
  }
}

async function saveSystemMemory() {
  if (!newMemoryContent.value.trim()) return
  memoryBusy.value = true
  try {
    await storeSystemMemory(newMemoryContent.value)
    newMemoryContent.value = ''
    await loadMemoryCore()
  } catch (error) {
    toast.value = error instanceof Error ? error.message : String(error)
    window.setTimeout(() => (toast.value = ''), 3200)
  } finally {
    memoryBusy.value = false
  }
}

async function runMemoryDiagnosis() {
  memoryBusy.value = true
  try {
    memoryDiagnoses.value = await diagnoseMemoryCore()
    if (!memoryDiagnoses.value.length) {
      toast.value = copy.value.diagnosisClear
      window.setTimeout(() => (toast.value = ''), 3200)
    }
  } catch (error) {
    toast.value = error instanceof Error ? error.message : String(error)
    window.setTimeout(() => (toast.value = ''), 3200)
  } finally {
    memoryBusy.value = false
  }
}
async function loadAiEnvironment() {
  aiEnvironmentState.value = { kind: 'loading' }
  try {
    const summary = await getAiEnvironment()
    aiEnvironmentState.value = summary
      ? { kind: 'ready', summary }
      : { kind: 'desktop_required' }
  } catch (error) {
    aiEnvironmentState.value = {
      kind: 'error',
      message: error instanceof Error ? error.message : String(error),
    }
  }
}

async function loadModelManagement() {
  modelManagementState.value = { kind: 'loading' }
  modelManagerError.value = ''
  try {
    const snapshot = await getModelManagementSnapshot()
    modelManagementState.value = snapshot
      ? { kind: 'ready', snapshot }
      : { kind: 'desktop_required' }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    modelManagementState.value = { kind: 'error', message }
    modelManagerError.value = message
  }
}

async function addModelStorage() {
  modelStorageBusy.value = true
  modelManagerError.value = ''
  try {
    const path = await chooseModelStorageDirectory()
    if (!path) return
    const displayName = path.split(/[\\/]/).filter(Boolean).at(-1) ?? (locale.value === 'ja' ? 'モデル保存先' : 'Model Storage')
    await addModelStorageLocation(displayName, path)
    await loadModelManagement()
    toast.value = locale.value === 'ja' ? 'モデル保存先を追加しました' : 'Model storage added'
  } catch (error) {
    modelManagerError.value = error instanceof Error ? error.message : String(error)
  } finally {
    modelStorageBusy.value = false
  }
}

async function makeDefaultModelStorage(storageId: string) {
  modelStorageBusy.value = true
  modelManagerError.value = ''
  try {
    await setDefaultModelStorage(storageId)
    await loadModelManagement()
  } catch (error) {
    modelManagerError.value = error instanceof Error ? error.message : String(error)
  } finally {
    modelStorageBusy.value = false
  }
}

async function rescanModels() {
  if (modelManagementState.value.kind === 'desktop_required') return
  modelStorageBusy.value = true
  modelManagerError.value = ''
  try {
    modelManagementState.value = { kind: 'ready', snapshot: await scanModelStorage() }
  } catch (error) {
    modelManagerError.value = error instanceof Error ? error.message : String(error)
  } finally {
    modelStorageBusy.value = false
  }
}

function capabilityLabel(capability: ModelRecord['capabilities'][number]) {
  const labels = locale.value === 'ja'
    ? { coding: 'コーディング', reasoning: '推論', review: 'レビュー', general: '汎用', tool_use: 'ツール', structured_output: '構造化出力', long_context: '長文脈' }
    : { coding: 'Coding', reasoning: 'Reasoning', review: 'Review', general: 'General', tool_use: 'Tool use', structured_output: 'Structured output', long_context: 'Long context' }
  return labels[capability]
}

function compatibilityLabel(modelId: string) {
  const state = modelCompatibility.value.get(modelId)?.state ?? 'unknown'
  const labels = locale.value === 'ja'
    ? { compatible: '使用可能', compatible_with_offload: 'RAM併用で使用可能', resource_constrained: 'リソース不足', unsupported: '未対応', unknown: '判定保留' }
    : { compatible: 'Compatible', compatible_with_offload: 'Compatible with RAM', resource_constrained: 'Resource constrained', unsupported: 'Unsupported', unknown: 'Unknown' }
  return labels[state]
}

function modelRuntimeLabel(model: ModelRecord) {
  return model.runtime_compatibility
    .filter((runtime) => ['available', 'compatible', 'planned'].includes(runtime.state))
    .map((runtime) => runtime.runtime_id === 'vertex-built-in' ? 'Vertex Built-in' : runtime.runtime_id === 'ollama' ? 'Ollama' : runtime.runtime_id)
    .join(' / ') || '—'
}
async function releaseModel(providerId: string, modelId: string) {
  unloadingModel.value = modelId
  try {
    await unloadLocalModel(providerId, modelId)
    toast.value = copy.value.memoryReleased
    window.setTimeout(() => (toast.value = ''), 3200)
    await loadAiEnvironment()
  } catch (error) {
    toast.value = error instanceof Error ? error.message : String(error)
    window.setTimeout(() => (toast.value = ''), 3200)
  } finally {
    unloadingModel.value = ''
  }
}
function formatBytes(value: number) {
  if (value <= 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const index = Math.min(Math.floor(Math.log(value) / Math.log(1024)), units.length - 1)
  return `${(value / 1024 ** index).toFixed(index < 3 ? 0 : 1)} ${units[index]}`
}
function downloadPercent(job: ModelDownloadJob) {
  if (!job.total_bytes || job.total_bytes <= 0) return 0
  return Math.min(100, Math.round((job.completed_bytes / job.total_bytes) * 100))
}
function storageUsedPercent(total: number | null, available: number | null) {
  if (!total || available == null || total <= 0) return 0
  return Math.min(100, Math.max(0, Math.round(((total - available) / total) * 100)))
}
function downloadStateLabel(job: ModelDownloadJob) {
  if (job.state === 'queued') return copy.value.downloadQueued
  if (job.state === 'running') return copy.value.downloadRunning
  if (job.state === 'cancelling') return copy.value.downloadCancelling
  if (job.state === 'succeeded') return copy.value.downloadSucceeded
  if (job.state === 'cancelled') return copy.value.downloadCancelled
  if (job.state === 'interrupted') return copy.value.downloadInterrupted
  return copy.value.downloadFailed
}
function scheduleDownloadPoll() {
  window.clearTimeout(downloadPollTimer)
  if (activePage.value !== 'Models') return
  if (!downloadJobs.value.some((job) => ['queued', 'running', 'cancelling'].includes(job.state))) return
  downloadPollTimer = window.setTimeout(() => void loadDownloadJobs(), 1000)
}
async function loadDownloadJobs() {
  try {
    downloadJobs.value = await listModelDownloads()
    scheduleDownloadPoll()
    if (downloadJobs.value.some((job) => job.state === 'succeeded')) {
      if (localAiState.value.kind === 'ready') void loadLocalAi()
      if (aiEnvironmentState.value.kind === 'ready') void loadAiEnvironment()
    }
  } catch (error) {
    toast.value = error instanceof Error ? error.message : String(error)
    window.setTimeout(() => (toast.value = ''), 3200)
  }
}
async function beginModelDownload() {
  const modelId = downloadModelName.value.trim()
  if (!modelId) return
  downloadBusy.value = true
  try {
    await startModelDownload('ollama', modelId)
    downloadModelName.value = ''
    toast.value = copy.value.downloadStarted
    window.setTimeout(() => (toast.value = ''), 3200)
    await loadDownloadJobs()
  } catch (error) {
    toast.value = error instanceof Error ? error.message : String(error)
    window.setTimeout(() => (toast.value = ''), 3200)
  } finally {
    downloadBusy.value = false
  }
}
async function stopModelDownload(jobId: string) {
  try {
    await cancelModelDownload(jobId)
    await loadDownloadJobs()
  } catch (error) {
    toast.value = error instanceof Error ? error.message : String(error)
    window.setTimeout(() => (toast.value = ''), 3200)
  }
}
async function loadLocalAi() {
  localAiState.value = { kind: 'loading' }
  const result = await getLocalProviderStatus()
  if (result.kind === 'desktop_required') {
    localAiState.value = { kind: 'desktop_required' }
    return
  }
  if (result.kind === 'error') {
    localAiState.value = {
      kind: 'error',
      message: result.error.human_fallback_message,
      technical: `${result.error.machine_readable_code}: ${result.error.technical_message}`,
    }
    return
  }
  localAiState.value = { kind: 'ready', status: result.status }
  const available = result.status.models.map((model) => model.reference.model_id)
  if (!available.includes(selectedLocalModel.value)) selectedLocalModel.value = available[0] ?? ''
}
async function runLocalTest() {
  if (!selectedLocalModel.value || !localPrompt.value.trim()) {
    localRunError.value = copy.value.promptRequired
    return
  }
  localRunPending.value = true
  localRunError.value = ''
  localResponse.value = null
  const result = await generateLocal({
    model_id: selectedLocalModel.value,
    prompt: localPrompt.value,
    temperature: 0.2,
    max_output_tokens: 2048,
  })
  localRunPending.value = false
  if (result.kind === 'ready') localResponse.value = result.response
  else if (result.kind === 'desktop_required') localRunError.value = copy.value.desktopAiRequired
  else localRunError.value = `${result.error.human_fallback_message} (${result.error.machine_readable_code})`
}
function categoryLabel(category: SystemAsset['category']) {
  const labels = {
    ai: copy.value.categoryAi, developer: copy.value.categoryDeveloper, creator: copy.value.categoryCreator,
    runtime: copy.value.categoryRuntime, database: copy.value.categoryDatabase, server: copy.value.categoryServer,
    system: copy.value.categorySystem, hardware: copy.value.categoryHardware, storage: copy.value.categoryStorage,
  }
  return labels[category]
}
function memoryCategoryLabel(category: string) {
  const labels: Record<string, string> = {
    project: copy.value.project,
    decision: copy.value.decision,
    knowledge: copy.value.knowledge,
    experience: copy.value.experience,
    system: locale.value === 'ja' ? 'システム' : 'System',
    working: locale.value === 'ja' ? '作業中' : 'Working',
    conversation: locale.value === 'ja' ? '会話' : 'Conversation',
    long_term: locale.value === 'ja' ? '長期記憶' : 'Long-term',
  }
  return labels[category] ?? category
}
function developerStateLabel(state: string) {
  if (locale.value === 'en') return state.replaceAll('_', ' ')
  const labels: Record<string, string> = {
    QUEUED: '待機中', ANALYZING: '解析中', PLANNING: '計画中', IMPLEMENTING: '実装中',
    BUILDING: 'ビルド中', TESTING: 'テスト中', FIXING: '修正中', REVIEWING: '確認中',
    WAITING_APPROVAL: '承認待ち', COMPLETED: '完了', FAILED: '失敗', CANCELLED: 'キャンセル済み',
  }
  return labels[state] ?? state
}
function developerRiskLabel(risk: string) {
  if (locale.value === 'en') return risk
  return ({ LOW: '低', MEDIUM: '中', HIGH: '高', CRITICAL: '重大' } as Record<string, string>)[risk] ?? risk
}
function kindLabel(kind: string) {
  const labels: Record<string, string> = {
    application: copy.value.kindApplication, executable: copy.value.kindExecutable, runtime: copy.value.kindRuntime,
    service: copy.value.kindService, process: copy.value.kindProcess, dependency: copy.value.kindDependency,
    sdk: copy.value.kindSdk, driver: copy.value.kindDriver, storage_device: copy.value.kindStorageDevice, configuration: copy.value.kindConfiguration,
  }
  return labels[kind] ?? kind
}
function observedLabel(value: string) {
  return new Intl.DateTimeFormat(locale.value === 'ja' ? 'ja-JP' : 'en-US', { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(value))
}
function setLocale(next: Locale) {
  if (locale.value === next) return
  locale.value = next
  toast.value = copy.value.languageChanged
  window.setTimeout(() => (toast.value = ''), 3200)
}
function providerTitle() {
  return locale.value === 'ja' ? `${selectedProvider.value}${copy.value.connectProvider}` : `${copy.value.connectProvider}${selectedProvider.value}`
}
function connectProvider() {
  toast.value = apiKey.value
    ? (locale.value === 'ja' ? `${selectedProvider.value}の${copy.value.validationQueued}` : `${selectedProvider.value}${copy.value.validationQueued}`)
    : copy.value.apiKeyRequired
  if (apiKey.value) {
    apiKey.value = ''
    providerDialog.value = false
  }
  window.setTimeout(() => (toast.value = ''), 3200)
}
</script>

<template>
  <div class="app-shell">
    <aside class="sidebar" :class="{ open: sidebarOpen }">
      <div class="brand-row">
        <img class="brand-lockup" src="/vertex-ai-lockup.png" alt="Vertex AI" />
        <button class="icon-button mobile-close" :aria-label="copy.closeNavigation" @click="sidebarOpen = false"><X :size="18" /></button>
      </div>

      <nav class="navigation" :aria-label="copy.primaryNavigation">
        <template v-for="item in navItems" :key="item.key">
          <p v-if="item.group" class="nav-group">{{ groupLabel(item.group) }}</p>
          <button class="nav-item" :class="{ active: activePage === item.key }" @click="navigate(item.key)">
            <component :is="item.icon" :size="17" :stroke-width="1.8" />
            <span>{{ navLabel(item.key) }}</span>
            <span v-if="item.key === 'Logs'" class="nav-count">12</span>
          </button>
        </template>
      </nav>

      <div class="sidebar-footer">
        <div class="core-mini-status"><span class="status-orb"></span><div><strong>{{ copy.coreOnline }}</strong><span>{{ copy.localVersion }}</span></div><ChevronRight :size="16" /></div>
        <div class="profile-row"><div class="avatar">AF</div><div><strong>{{ copy.administrator }}</strong><span>{{ copy.localWorkspace }}</span></div><MoreHorizontal :size="17" /></div>
      </div>
    </aside>

    <div v-if="sidebarOpen" class="sidebar-scrim" @click="sidebarOpen = false"></div>

    <main class="main-area">
      <header class="topbar">
        <button class="icon-button menu-button" :aria-label="copy.openNavigation" @click="sidebarOpen = true"><Menu :size="19" /></button>
        <button class="command-search" @click="commandOpen = true"><Search :size="16" /><span>{{ copy.searchGlobal }}</span><kbd><Command :size="12" /> K</kbd></button>
        <div class="topbar-actions">
          <span class="preview-pill">{{ copy.previewData }}</span>
          <button class="icon-button" :aria-label="copy.notifications"><Bell :size="18" /><i></i></button>
          <button class="environment-button"><span></span>{{ copy.localCore }}<ChevronDown :size="14" /></button>
        </div>
      </header>

      <div class="content-wrap">
        <div class="page-heading">
          <div><p class="eyebrow">{{ copy.management }}</p><h1>{{ activePageLabel }}</h1><p>{{ pageDescription }}</p></div>
          <div class="heading-actions">
            <button class="button secondary" @click="refreshPage"><RefreshCw :size="15" :class="{ rotating: activePage === 'Environment Explorer' && environmentRefreshing }" />{{ copy.refresh }}</button>
            <button v-if="activePage === 'Providers' || activePage === 'Dashboard'" class="button primary" @click="providerDialog = true"><Plus :size="16" />{{ copy.addProvider }}</button>
          </div>
        </div>

        <template v-if="activePage === 'Dashboard'">
          <section class="metrics-grid">
            <article class="metric-card"><div class="metric-top"><span class="metric-icon violet"><BrainCircuit :size="18" /></span><span class="trend" :class="dashboardInstalledModels !== null ? 'up' : 'neutral'">{{ dashboardInstalledModels !== null ? copy.verifiedLive : copy.dataUnavailable }}</span></div><p>{{ copy.availableModels }}</p><strong>{{ dashboardInstalledModels ?? '—' }}</strong><span>Ollama · {{ dashboardRuntime?.version ?? copy.dataUnavailable }}</span></article>
            <article class="metric-card"><div class="metric-top"><span class="metric-icon cyan"><Power :size="18" /></span><span class="trend" :class="dashboardLoadedModels ? 'up' : 'neutral'">{{ dashboardLoadedModels ? copy.operational : copy.currentState }}</span></div><p>{{ copy.activeModels }}</p><strong>{{ dashboardLoadedModels ?? '—' }}</strong><span>{{ copy.loadedModel }} · {{ formatBytes(aiEnvironmentState.kind === 'ready' ? aiEnvironmentState.summary.total_vram_bytes : 0) }}</span></article>
            <article class="metric-card"><div class="metric-top"><span class="metric-icon blue"><Clock3 :size="18" /></span><span class="trend" :class="activeDownloads.length ? 'up' : 'neutral'">{{ activeDownloads.length ? copy.downloadRunning : copy.currentState }}</span></div><p>{{ copy.runningTasks }}</p><strong>{{ activeDownloads.length }}</strong><span>{{ activeDownloads.length ? copy.activeDownloads : copy.noRunningAiTasks }}</span></article>
            <article class="metric-card"><div class="metric-top"><span class="metric-icon green"><CircleGauge :size="18" /></span><span class="trend" :class="dashboardIssueCount === 0 ? 'up' : 'neutral'">{{ dashboardIssueCount === null ? copy.notMeasured : `${dashboardIssueCount} ${copy.issues}` }}</span></div><p>{{ copy.systemHealthScore }}</p><strong>{{ dashboardHealthScore ?? '—' }}<small v-if="dashboardHealthScore !== null"> / 100</small></strong><span>{{ dashboardAction.title }}</span></article>
          </section>

          <section class="cockpit-grid">
            <article class="panel operational-center">
              <div class="panel-header cockpit-header"><div><p class="eyebrow">Operational Cockpit</p><h2>{{ copy.operationalCenter }}</h2><p>{{ copy.operationalCenterDescription }}</p></div><span class="live-badge"><i></i>{{ aiEnvironmentState.kind === 'ready' ? copy.verifiedLive : copy.dataUnavailable }}</span></div>
              <div class="operation-list">
                <div class="operation-row"><span class="operation-icon"><Zap :size="17" /></span><div><strong>Vertex AI Core</strong><span>Desktop Command Core</span></div><b class="status healthy"><i></i>{{ copy.operational }}</b><small>v0.1.2</small></div>
                <div class="operation-row"><span class="operation-icon"><Cpu :size="17" /></span><div><strong>Ollama</strong><span>{{ dashboardRuntime?.endpoint ?? '127.0.0.1:11434' }}</span></div><b class="status" :class="dashboardRuntime?.health === 'ready' ? 'healthy' : dashboardRuntime ? 'danger' : 'neutral'"><i></i>{{ dashboardRuntime?.health === 'ready' ? copy.runtimeReady : dashboardRuntime ? copy.offlineStatus : copy.dataUnavailable }}</b><small>{{ dashboardRuntime?.version ?? '—' }}</small></div>
                <div class="operation-row"><span class="operation-icon"><Database :size="17" /></span><div><strong>PostgreSQL</strong><span>Memory / Knowledge backend</span></div><b class="status neutral"><i></i>{{ copy.apiNotConnected }}</b><small>—</small></div>
                <div class="operation-row"><span class="operation-icon"><Network :size="17" /></span><div><strong>{{ copy.knowledgeCore }}</strong><span>{{ copy.connectionPending }}</span></div><b class="status neutral"><i></i>{{ copy.dataUnavailable }}</b><small>—</small></div>
              </div>
              <footer class="cockpit-summary"><div><span>{{ copy.systemHealthScore }}</span><strong>{{ dashboardHealthScore ?? '—' }}<small v-if="dashboardHealthScore !== null"> / 100</small></strong></div><div><span>{{ copy.issues }}</span><strong>{{ dashboardIssueCount ?? '—' }}</strong></div><div><span>{{ copy.recommendations }}</span><strong>{{ dashboardAction.tone === 'healthy' ? 0 : 1 }}</strong></div><div><span>{{ copy.runningTasks }}</span><strong>{{ activeDownloads.length }}</strong></div></footer>
            </article>

            <article class="panel action-center" :class="`tone-${dashboardAction.tone}`">
              <div class="action-label"><AlertTriangle v-if="dashboardAction.tone !== 'healthy'" :size="15" /><Check v-else :size="15" />{{ copy.actionRequired }}</div>
              <div class="action-main"><span class="action-symbol"><HardDrive v-if="dashboardAction.tone === 'warning'" :size="24" /><Activity v-else-if="dashboardAction.tone === 'danger'" :size="24" /><ShieldCheck v-else :size="24" /></span><h2>{{ dashboardAction.title }}</h2><p>{{ dashboardAction.detail }}</p></div>
              <div v-if="dashboardRuntime?.storage_total_bytes" class="action-fact"><span>{{ copy.modelLocation }}</span><strong class="mono">{{ dashboardRuntime.model_storage_path ?? '—' }}</strong><span>{{ copy.diskFree }} {{ formatBytes(dashboardRuntime.storage_available_bytes ?? 0) }}</span></div>
              <div class="recommendation-block"><span><Sparkles :size="14" />{{ copy.recommendations }}</span><p>{{ dashboardAction.tone === 'healthy' ? copy.recommendationHealthy : dashboardAction.detail }}</p></div>
              <button class="button secondary" @click="navigate(dashboardAction.page)">{{ copy.viewDetails }}<ChevronRight :size="15" /></button>
            </article>
          </section>

          <section class="operations-secondary-grid">
            <article class="panel resource-panel">
              <div class="panel-header"><div><h2>{{ copy.resourceMonitor }}</h2><p>{{ copy.resourceMonitorDescription }}</p></div><span class="placeholder-chip">{{ copy.telemetryPending }}</span></div>
              <div class="resource-list">
                <div><span>CPU</span><strong>—</strong><div class="meter"><i></i></div><small>{{ copy.notMeasured }}</small></div>
                <div><span>RAM</span><strong>—</strong><div class="meter"><i></i></div><small>{{ copy.notMeasured }}</small></div>
                <div><span>GPU</span><strong>—</strong><div class="meter"><i></i></div><small>{{ copy.notMeasured }}</small></div>
                <div><span>{{ copy.vramAllocated }}</span><strong>{{ aiEnvironmentState.kind === 'ready' ? formatBytes(aiEnvironmentState.summary.total_vram_bytes) : '—' }}</strong><div class="meter volume"><i :class="{ observed: aiEnvironmentState.kind === 'ready' && aiEnvironmentState.summary.total_vram_bytes > 0 }"></i></div><small>{{ aiEnvironmentState.kind === 'ready' ? copy.verifiedLive : copy.notMeasured }}</small></div>
                <div><span>{{ copy.storageStatus }}</span><strong>{{ dashboardRuntime?.storage_total_bytes ? `${dashboardStoragePercent}%` : '—' }}</strong><div class="meter actual" :class="{ warning: dashboardStoragePercent >= 90 }"><i :style="{ width: `${dashboardStoragePercent}%` }"></i></div><small>{{ dashboardRuntime?.storage_available_bytes != null ? `${formatBytes(dashboardRuntime.storage_available_bytes)} ${copy.diskFree}` : copy.notMeasured }}</small></div>
              </div>
            </article>

            <article class="panel active-models-panel">
              <div class="panel-header"><div><h2>{{ copy.activeModels }}</h2><p>{{ copy.runtimeStatus }}</p></div><button class="text-button inline" @click="navigate('Models')">{{ copy.viewDetails }}<ChevronRight :size="14" /></button></div>
              <div v-if="dashboardRuntime?.installed_models.length" class="dashboard-model-list"><div v-for="model in dashboardRuntime.installed_models.slice(0, 4)" :key="model.reference.model_id"><span class="model-avatar"><BrainCircuit :size="16" /></span><p><strong>{{ model.display_name }}</strong><span>LOCAL · {{ model.parameter_size ?? '—' }} · {{ model.quantization_level ?? '—' }}</span></p><b :class="dashboardRuntime.loaded_models.some((loaded) => loaded.reference.model_id === model.reference.model_id) ? 'loaded' : ''">{{ dashboardRuntime.loaded_models.some((loaded) => loaded.reference.model_id === model.reference.model_id) ? copy.loadedModel : copy.providerReady }}</b></div></div>
              <div v-else class="dashboard-empty"><BrainCircuit :size="20" /><span>{{ aiEnvironmentState.kind === 'desktop_required' ? copy.inspectInDesktop : copy.noLocalModels }}</span></div>
            </article>

            <article class="panel ai-activity-panel">
              <div class="panel-header"><div><h2>{{ copy.aiActivity }}</h2><p>{{ copy.aiActivityDescription }}</p></div><span v-if="activeDownloads.length" class="live-badge"><i></i>LIVE</span></div>
              <div v-if="activeDownloads.length" class="agent-activity-list"><div v-for="job in activeDownloads" :key="job.id"><span class="agent-orb"><Bot :size="15" /></span><p><strong>Ollama · {{ job.model_id }}</strong><span>{{ copy.activeDownloads }} · {{ downloadStateLabel(job) }}</span></p><b>{{ job.total_bytes ? `${downloadPercent(job)}%` : '…' }}</b></div></div>
              <div v-else class="dashboard-empty compact"><Check :size="18" /><span>{{ copy.noRunningAiTasks }}</span></div>
              <footer class="future-connection"><Clock3 :size="14" /><span>Codex / Reviewer / Judge / Council</span><b>{{ copy.futureAgentConnection }}</b></footer>
            </article>
          </section>

          <section class="dashboard-aux-grid">
            <article class="panel traffic-panel compact-chart-panel">
              <div class="panel-header"><div><h2>{{ copy.intelligenceActivity }}</h2><p>{{ copy.routedByLocation }} · {{ copy.previewData }}</p></div><button class="small-select">{{ copy.last7Days }}<ChevronDown :size="13" /></button></div>
              <div class="legend"><span><i class="legend-local"></i>{{ copy.localModels }}</span><span><i class="legend-cloud"></i>{{ copy.cloudModels }}</span></div>
              <div class="chart compact-chart" :aria-label="copy.activityChart"><div v-for="(height, index) in [42,55,38,65,58,74,69,82,61,86,76,91,84,96]" :key="index" class="bar-pair"><i class="bar local" :style="{ height: `${height}%` }"></i><i class="bar cloud" :style="{ height: `${Math.max(18, height - 27)}%` }"></i></div></div>
              <div class="chart-labels"><span>{{ copy.aug06 }}</span><span>{{ copy.aug08 }}</span><span>{{ copy.aug10 }}</span><span>{{ copy.today }}</span></div>
            </article>

            <article class="panel health-panel strengthened-health">
              <div class="panel-header"><div><h2>{{ copy.systemHealth }}</h2><p>{{ copy.liveStatus }}</p></div><span class="health-score-small">{{ dashboardHealthScore ?? '—' }}<small v-if="dashboardHealthScore !== null">/100</small></span></div>
              <div class="health-list">
                <div><span class="health-icon"><Zap :size="16" /></span><p><strong>Vertex AI Core</strong><span>v0.1.2</span></p><b>{{ copy.healthy }}</b></div>
                <div><span class="health-icon"><Cpu :size="16" /></span><p><strong>Ollama</strong><span>{{ dashboardRuntime?.checked_at ? observedLabel(dashboardRuntime.checked_at) : copy.dataUnavailable }}</span></p><b :class="{ warning: dashboardRuntime?.health !== 'ready' }">{{ dashboardRuntime?.health === 'ready' ? copy.healthy : copy.dataUnavailable }}</b></div>
                <div><span class="health-icon"><Database :size="16" /></span><p><strong>PostgreSQL</strong><span>{{ copy.connectionPending }}</span></p><b class="neutral-status">{{ copy.dataUnavailable }}</b></div>
                <div><span class="health-icon"><HardDrive :size="16" /></span><p><strong>{{ copy.storageStatus }}</strong><span>{{ dashboardRuntime?.model_storage_path ?? copy.dataUnavailable }}</span></p><b :class="{ warning: dashboardStoragePercent >= 90, 'neutral-status': !dashboardRuntime?.storage_total_bytes }">{{ dashboardRuntime?.storage_total_bytes ? (dashboardStoragePercent >= 90 ? copy.warningStatus : copy.healthy) : copy.dataUnavailable }}</b></div>
              </div>
              <button class="text-button" @click="navigate('System Status')">{{ copy.viewSystemDetails }}<ChevronRight :size="14" /></button>
            </article>
          </section>

          <section class="panel routes-panel">
            <div class="panel-header"><div><h2>{{ copy.recentRouting }}</h2><p>{{ copy.selectionReason }} · {{ copy.previewData }}</p></div><button class="button tertiary" @click="navigate('Routing')"><ListFilter :size="15" />{{ copy.viewRouting }}</button></div>
            <div class="table-wrap"><table><thead><tr><th>{{ copy.time }}</th><th>{{ copy.application }}</th><th>{{ copy.task }}</th><th>{{ copy.selectedModel }}</th><th>{{ copy.policy }}</th><th>{{ copy.latency }}</th><th>{{ copy.status }}</th></tr></thead><tbody><tr v-for="route in routes" :key="route.time"><td class="mono">{{ route.time }}</td><td>{{ route.app }}</td><td>{{ route.task }}</td><td><span class="model-cell"><Bot :size="14" />{{ route.model }}</span></td><td><span class="mode-tag">{{ route.mode }}</span></td><td>{{ route.latency }}</td><td><span class="complete"><Check :size="12" />{{ copy.complete }}</span></td></tr></tbody></table></div>
          </section>
        </template>

        <template v-else-if="activePage === 'Providers'">
          <section class="provider-grid"><article v-for="provider in providers" :key="provider.name" class="provider-card"><div class="provider-head"><span class="provider-logo" :class="provider.tone">{{ provider.name.slice(0, 2) }}</span><button class="icon-button"><MoreHorizontal :size="18" /></button></div><h2>{{ provider.name }}</h2><p>{{ provider.detail }}</p><div class="provider-stats"><div><span>{{ copy.models }}</span><strong>{{ provider.models }}</strong></div><div><span>{{ copy.latency }}</span><strong>{{ provider.latency }}</strong></div></div><div class="provider-footer"><span :class="provider.configured ? 'connected' : 'not-configured'"><i></i>{{ provider.configured ? copy.connected : copy.notConfigured }}</span><button @click="selectedProvider = provider.name; providerDialog = true">{{ provider.configured ? copy.manage : copy.connect }}<ChevronRight :size="14" /></button></div></article></section>
        </template>

        <template v-else-if="activePage === 'Models'">
          <section class="model-manager-hero panel">
            <div><p class="eyebrow">Model Management Foundation</p><h2>{{ locale === 'ja' ? 'モデルマネージャー' : 'Model Manager' }}</h2><p>{{ locale === 'ja' ? 'モデル本体とRuntimeを分離し、保存場所・能力・このPCでの使用可否を一元管理します。' : 'Manage model files, runtimes, capabilities, storage, and PC compatibility independently.' }}</p></div>
            <div class="page-actions"><button class="button secondary" :disabled="modelStorageBusy" @click="rescanModels"><RefreshCw :size="15" />{{ locale === 'ja' ? '再スキャン' : 'Rescan' }}</button><button class="button primary" :disabled="modelStorageBusy || modelManagementState.kind === 'desktop_required'" @click="addModelStorage"><Plus :size="15" />{{ locale === 'ja' ? '保存先を追加' : 'Add storage' }}</button></div>
          </section>
          <p v-if="modelManagementState.kind === 'desktop_required'" class="model-manager-notice"><Cpu :size="15" />{{ locale === 'ja' ? '保存先の追加と実モデル検出はデスクトップ版で利用できます。' : 'Storage selection and live discovery are available in the desktop app.' }}</p>
          <p v-if="modelManagerError || modelManagementState.kind === 'error'" class="download-error model-manager-error"><AlertTriangle :size="14" />{{ modelManagerError || (modelManagementState.kind === 'error' ? modelManagementState.message : '') }}</p>

          <section class="model-manager-summary">
            <article class="panel"><span class="metric-icon blue"><BrainCircuit :size="18" /></span><p><span>{{ locale === 'ja' ? '登録モデル' : 'Registered models' }}</span><strong>{{ modelManagementState.kind === 'ready' ? modelManagementState.snapshot.models.length : '—' }}</strong></p></article>
            <article class="panel"><span class="metric-icon violet"><HardDrive :size="18" /></span><p><span>{{ locale === 'ja' ? '保存先' : 'Storage locations' }}</span><strong>{{ modelManagementState.kind === 'ready' ? modelManagementState.snapshot.storage_locations.length : '—' }}</strong></p></article>
            <article class="panel"><span class="metric-icon cyan"><MemoryStick :size="18" /></span><p><span>{{ locale === 'ja' ? '利用可能RAM' : 'Available RAM' }}</span><strong>{{ modelManagementState.kind === 'ready' && modelManagementState.snapshot.hardware.system_ram_available ? formatBytes(modelManagementState.snapshot.hardware.system_ram_available) : '—' }}</strong></p></article>
            <article class="panel"><span class="metric-icon green"><Check :size="18" /></span><p><span>{{ locale === 'ja' ? '使用可能判定' : 'Compatible' }}</span><strong>{{ modelManagementState.kind === 'ready' ? modelManagementState.snapshot.compatibility.filter(item => ['compatible', 'compatible_with_offload'].includes(item.state)).length : '—' }}</strong></p></article>
          </section>

          <section class="model-manager-grid">
            <article class="panel model-storage-panel">
              <div class="panel-header"><div><h2>{{ locale === 'ja' ? '保存先' : 'Storage Locations' }}</h2><p>{{ locale === 'ja' ? '内蔵・外付けドライブを複数登録できます' : 'Register multiple internal or external drives' }}</p></div><HardDrive :size="20" /></div>
              <div v-if="modelManagementState.kind === 'ready' && modelManagementState.snapshot.storage_locations.length" class="model-storage-list">
                <div v-for="storage in modelManagementState.snapshot.storage_locations" :key="storage.id" class="model-storage-row">
                  <span class="model-storage-icon"><HardDrive :size="17" /></span>
                  <p><strong>{{ storage.display_name }}<b v-if="storage.is_default">{{ locale === 'ja' ? '既定' : 'Default' }}</b></strong><span class="mono">{{ storage.path }}</span><small>{{ locale === 'ja' ? '空き' : 'Free' }} {{ storage.free_space !== null ? formatBytes(storage.free_space) : '—' }} / {{ storage.total_space !== null ? formatBytes(storage.total_space) : '—' }}</small></p>
                  <div><span class="status-chip" :class="storage.availability === 'available' ? 'healthy' : 'warning'">{{ storage.availability === 'available' ? (locale === 'ja' ? '利用可能' : 'Available') : (locale === 'ja' ? '未接続' : 'Unavailable') }}</span><button v-if="!storage.is_default" class="text-button inline" :disabled="modelStorageBusy" @click="makeDefaultModelStorage(storage.id)">{{ locale === 'ja' ? '既定にする' : 'Make default' }}</button></div>
                </div>
              </div>
              <div v-else class="runtime-empty"><HardDrive :size="18" /><span>{{ locale === 'ja' ? '保存先はまだ登録されていません' : 'No storage locations registered' }}</span></div>
            </article>

            <article class="panel model-hardware-panel">
              <div class="panel-header"><div><h2>{{ locale === 'ja' ? 'ハードウェア' : 'Hardware' }}</h2><p>{{ locale === 'ja' ? 'Compatibility判定に使用する実測値' : 'Observed facts used for compatibility' }}</p></div><Cpu :size="20" /></div>
              <div class="hardware-facts">
                <div><span>{{ locale === 'ja' ? 'システムRAM' : 'System RAM' }}</span><strong>{{ modelManagementState.kind === 'ready' && modelManagementState.snapshot.hardware.system_ram_total ? formatBytes(modelManagementState.snapshot.hardware.system_ram_total) : '—' }}</strong><small>{{ locale === 'ja' ? '利用可能' : 'Available' }} {{ modelManagementState.kind === 'ready' && modelManagementState.snapshot.hardware.system_ram_available ? formatBytes(modelManagementState.snapshot.hardware.system_ram_available) : '—' }}</small></div>
                <div><span>{{ locale === 'ja' ? '使用中VRAM' : 'VRAM in use' }}</span><strong>{{ modelManagementState.kind === 'ready' ? formatBytes(modelManagementState.snapshot.hardware.gpu_vram_in_use) : '—' }}</strong><small>{{ locale === 'ja' ? 'Runtime実測値' : 'Runtime observation' }}</small></div>
                <div><span>{{ locale === 'ja' ? 'GPU総VRAM' : 'Total GPU VRAM' }}</span><strong>{{ modelManagementState.kind === 'ready' && modelManagementState.snapshot.hardware.gpu_vram_total ? formatBytes(modelManagementState.snapshot.hardware.gpu_vram_total) : '—' }}</strong><small>{{ locale === 'ja' ? '未検出時はRAM基準で判定' : 'RAM fallback when unavailable' }}</small></div>
              </div>
            </article>
          </section>

          <section class="panel data-panel model-registry-panel">
            <div class="panel-header"><div><h2>{{ locale === 'ja' ? 'インストール済みモデル' : 'Installed Models' }}</h2><p>{{ locale === 'ja' ? 'Local StorageとRuntime Adapterから統合したRegistry' : 'Unified Registry from local storage and runtime adapters' }}</p></div><span class="live-badge"><i></i>{{ modelManagementState.kind === 'ready' ? (locale === 'ja' ? '実データ' : 'Live') : (locale === 'ja' ? '待機中' : 'Waiting') }}</span></div>
            <div class="table-wrap"><table><thead><tr><th>{{ locale === 'ja' ? 'モデル' : 'Model' }}</th><th>{{ locale === 'ja' ? '得意分野' : 'Capabilities' }}</th><th>Runtime</th><th>{{ locale === 'ja' ? '保存場所' : 'Storage' }}</th><th>{{ locale === 'ja' ? 'サイズ' : 'Size' }}</th><th>{{ locale === 'ja' ? 'このPC' : 'This PC' }}</th></tr></thead><tbody><tr v-for="model in models" :key="model.id"><td><span class="model-cell"><BrainCircuit :size="15" /><span><strong>{{ model.display_name }}</strong><small>{{ model.family ?? model.format ?? '—' }} · {{ model.parameter_size ?? model.quantization ?? '—' }}</small></span></span></td><td><div class="capability-tags"><span v-for="capability in model.capabilities.slice(0, 3)" :key="capability">{{ capabilityLabel(capability) }}</span></div></td><td><span class="mode-tag">{{ modelRuntimeLabel(model) }}</span></td><td class="model-path-cell" :title="model.storage_path ?? ''">{{ model.storage_path ?? '—' }}</td><td>{{ model.file_size !== null ? formatBytes(model.file_size) : '—' }}</td><td><span class="compatibility-chip" :class="modelCompatibility.get(model.id)?.state">{{ compatibilityLabel(model.id) }}</span></td></tr><tr v-if="!models.length"><td colspan="6"><div class="runtime-empty"><BrainCircuit :size="18" /><span>{{ locale === 'ja' ? '登録済みモデルはありません。保存先を追加するかOllamaを起動してください。' : 'No registered models. Add storage or start Ollama.' }}</span></div></td></tr></tbody></table></div>
          </section>

          <section class="panel download-panel">
            <div class="download-heading"><div><p class="eyebrow">Runtime Adapter · Ollama</p><h2>{{ copy.downloadModel }}</h2></div></div>
            <form class="download-form" @submit.prevent="beginModelDownload">
              <label><span>{{ copy.modelName }}</span><input v-model="downloadModelName" :placeholder="copy.modelNameHint" autocomplete="off" /></label>
              <button class="button primary" :disabled="downloadBusy || !downloadModelName.trim()"><HardDrive :size="16" />{{ downloadBusy ? copy.downloading : copy.download }}</button>
            </form>
            <div class="download-section">
              <h3>{{ copy.downloadJobs }}</h3>
              <div v-if="downloadJobs.length" class="download-list">
                <article v-for="job in downloadJobs" :key="job.id" class="download-job">
                  <div class="download-job-head"><div><strong>{{ job.model_id }}</strong><span>{{ downloadStateLabel(job) }}</span></div><button v-if="['queued', 'running', 'cancelling'].includes(job.state)" class="button secondary" :disabled="job.state === 'cancelling'" @click="stopModelDownload(job.id)"><X :size="14" />{{ copy.cancelDownload }}</button></div>
                  <div class="progress-track" :class="job.state"><i :style="{ width: `${downloadPercent(job)}%` }"></i></div>
                  <div class="download-job-meta"><span>{{ formatBytes(job.completed_bytes) }}<template v-if="job.total_bytes"> / {{ formatBytes(job.total_bytes) }}</template></span><strong>{{ job.total_bytes ? `${downloadPercent(job)}%` : job.status }}</strong></div>
                  <p v-if="job.error_message" class="download-error">{{ job.error_message }}</p>
                </article>
              </div>
              <p v-else class="runtime-empty">{{ copy.noDownloadJobs }}</p>
            </div>
          </section>
        </template>

        <template v-else-if="activePage === 'AI Environment' || activePage === 'Local Runtimes'">
          <section v-if="aiEnvironmentState.kind === 'idle' || aiEnvironmentState.kind === 'loading'" class="panel environment-message">
            <span class="environment-pulse"><Cpu :size="25" /></span><h2>{{ copy.environmentLoading }}</h2>
          </section>
          <section v-else-if="aiEnvironmentState.kind === 'desktop_required'" class="panel environment-message">
            <span class="feature-icon"><Cpu :size="25" /></span><h2>{{ copy.desktopRequired }}</h2><p>{{ copy.desktopAiRequired }}</p>
          </section>
          <section v-else-if="aiEnvironmentState.kind === 'error'" class="panel environment-message error-state">
            <span class="feature-icon"><Activity :size="25" /></span><h2>{{ copy.environmentUnavailable }}</h2><p>{{ aiEnvironmentState.message }}</p><button class="button primary" @click="loadAiEnvironment"><RefreshCw :size="15" />{{ copy.retry }}</button>
          </section>
          <template v-else-if="aiEnvironmentState.kind === 'ready'">
            <section v-if="activePage === 'AI Environment'" class="environment-summary ai-summary">
              <article class="panel"><span class="metric-icon green"><Power :size="18" /></span><p><span>{{ copy.runtimeOverview }}</span><strong>{{ aiEnvironmentState.summary.ready_runtime_count }}/{{ aiEnvironmentState.summary.runtime_count }}</strong></p></article>
              <article class="panel"><span class="metric-icon blue"><BrainCircuit :size="18" /></span><p><span>{{ copy.installedModels }}</span><strong>{{ aiEnvironmentState.summary.installed_model_count }}</strong></p></article>
              <article class="panel"><span class="metric-icon violet"><HardDrive :size="18" /></span><p><span>{{ copy.totalModelSize }}</span><strong>{{ formatBytes(aiEnvironmentState.summary.total_model_bytes) }}</strong></p></article>
              <article class="panel"><span class="metric-icon cyan"><Cpu :size="18" /></span><p><span>{{ copy.vramInUse }}</span><strong>{{ formatBytes(aiEnvironmentState.summary.total_vram_bytes) }}</strong></p></article>
            </section>

            <section class="runtime-list">
              <article v-for="runtime in aiEnvironmentState.summary.runtimes" :key="runtime.provider_id" class="panel runtime-card">
                <header class="runtime-header"><span class="runtime-icon"><Cpu :size="22" /></span><div><h2>{{ runtime.display_name }}</h2><p>{{ runtime.endpoint }}</p></div><span class="runtime-health" :class="runtime.health"><i></i>{{ runtime.health === 'ready' ? copy.runtimeReady : copy.runtimeOffline }}</span></header>
                <div class="runtime-facts">
                  <div><span>{{ copy.version }}</span><strong>{{ runtime.version ?? '—' }}</strong></div>
                  <div><span>{{ copy.installedModels }}</span><strong>{{ runtime.installed_models.length }}</strong></div>
                  <div><span>{{ copy.loadedModels }}</span><strong>{{ runtime.loaded_models.length }}</strong></div>
                  <div><span>{{ copy.checkedAt }}</span><strong>{{ observedLabel(runtime.checked_at) }}</strong></div>
                </div>
                <dl class="runtime-paths"><div><dt>{{ copy.executable }}</dt><dd class="mono">{{ runtime.executable_path ?? '—' }}</dd></div><div><dt>{{ copy.modelStorage }}</dt><dd class="mono">{{ runtime.model_storage_path ?? '—' }}</dd></div></dl>
                <div v-if="runtime.storage_total_bytes && runtime.storage_available_bytes !== null" class="storage-meter">
                  <div><span>{{ copy.storageCapacity }}</span><strong>{{ formatBytes(runtime.storage_total_bytes) }}</strong></div>
                  <div class="progress-track"><i :style="{ width: `${storageUsedPercent(runtime.storage_total_bytes, runtime.storage_available_bytes)}%` }"></i></div>
                  <p><span>{{ copy.storageUsed }} {{ storageUsedPercent(runtime.storage_total_bytes, runtime.storage_available_bytes) }}%</span><span>{{ copy.freeSpace }} {{ formatBytes(runtime.storage_available_bytes) }}</span></p>
                </div>

                <div v-if="activePage === 'AI Environment'" class="runtime-models">
                  <h3>{{ copy.localAiFacts }}</h3>
                  <div class="table-wrap"><table><thead><tr><th>{{ copy.model }}</th><th>{{ copy.modelDetails }}</th><th>{{ copy.context }}</th><th>{{ copy.totalModelSize }}</th><th>{{ copy.updated }}</th></tr></thead><tbody><tr v-for="model in runtime.installed_models" :key="model.reference.model_id"><td><span class="model-cell"><BrainCircuit :size="15" />{{ model.display_name }}</span></td><td>{{ model.family ?? '—' }} · {{ model.parameter_size ?? '—' }} · {{ model.quantization_level ?? '—' }}</td><td>{{ model.context_length?.toLocaleString() ?? '—' }}</td><td>{{ formatBytes(model.size_bytes) }}</td><td>{{ model.modified_at ? observedLabel(model.modified_at) : '—' }}</td></tr></tbody></table></div>
                </div>

                <div v-else class="runtime-models">
                  <h3>{{ copy.loadedModels }}</h3>
                  <div v-if="runtime.loaded_models.length" class="loaded-model-list">
                    <div v-for="model in runtime.loaded_models" :key="model.reference.model_id" class="loaded-model-row"><div><strong>{{ model.reference.model_id }}</strong><span>{{ copy.vramInUse }} {{ formatBytes(model.size_vram_bytes) }} · {{ copy.contextLength }} {{ model.context_length?.toLocaleString() ?? '—' }}</span></div><button class="button secondary" :disabled="unloadingModel === model.reference.model_id" @click="releaseModel(runtime.provider_id, model.reference.model_id)"><Power :size="14" />{{ unloadingModel === model.reference.model_id ? copy.releasingMemory : copy.releaseMemory }}</button></div>
                  </div>
                  <div v-else class="runtime-empty"><Check :size="18" /><span>{{ copy.noLoadedModels }}</span></div>
                </div>
              </article>
            </section>
          </template>
        </template>

        <template v-else-if="activePage === 'Memory'">
          <section v-if="memoryState.kind === 'loading' || memoryState.kind === 'idle'" class="panel environment-message"><span class="environment-pulse"><Database :size="25" /></span><h2>{{ copy.memoryCoreStarting }}</h2></section>
          <section v-else-if="memoryState.kind === 'desktop_required'" class="panel environment-message"><Database :size="25" /><h2>{{ copy.desktopRequired }}</h2><p>{{ copy.desktopRequiredDescription }}</p></section>
          <section v-else-if="memoryState.kind === 'error'" class="panel environment-message error-state"><AlertTriangle :size="25" /><h2>{{ copy.memoryCoreUnavailable }}</h2><p>{{ memoryState.message }}</p><div class="page-actions"><button class="button primary" @click="runMemoryAction('start')"><Power :size="15" />{{ copy.start }}</button><button class="button secondary" @click="runMemoryDiagnosis"><Activity :size="15" />{{ copy.diagnose }}</button></div></section>
          <template v-else>
            <section class="panel runtime-card">
              <div class="runtime-card-head"><div><span class="eyebrow">{{ copy.managedRuntime }}</span><h2>{{ copy.memoryCore }}</h2><p>{{ memoryState.status.state === 'READY' ? copy.memoryCoreReady : copy.memoryCoreUnavailable }}</p></div><span class="status-chip" :class="memoryState.status.state === 'READY' ? 'healthy' : 'warning'">{{ memoryState.status.state }}</span></div>
              <div class="page-actions"><button class="button primary" :disabled="memoryBusy || memoryState.status.state === 'READY'" @click="runMemoryAction('start')"><Power :size="15" />{{ copy.start }}</button><button class="button secondary" :disabled="memoryBusy || memoryState.status.state !== 'READY'" @click="runMemoryAction('stop')"><Power :size="15" />{{ copy.stop }}</button><button class="button secondary" :disabled="memoryBusy" @click="runMemoryAction('restart')"><RefreshCw :size="15" />{{ copy.restart }}</button><button class="button tertiary" :disabled="memoryBusy" @click="runMemoryDiagnosis"><Activity :size="15" />{{ copy.diagnose }}</button></div>
              <details class="runtime-details"><summary>{{ copy.memoryCoreDetails }}</summary><dl><div><dt>Engine</dt><dd>PostgreSQL {{ memoryState.status.version ?? '—' }}</dd></div><div><dt>Runtime</dt><dd class="mono">{{ memoryState.status.runtime_location }}</dd></div><div><dt>Data</dt><dd class="mono">{{ memoryState.status.data_location }}</dd></div><div><dt>Endpoint</dt><dd>{{ memoryState.status.host ?? '—' }}:{{ memoryState.status.port ?? '—' }}</dd></div></dl></details>
              <div v-if="memoryDiagnoses.length" class="environment-message error-state"><div v-for="finding in memoryDiagnoses" :key="finding.code"><strong>{{ finding.code }}</strong><p>{{ finding.detail }}</p></div></div>
            </section>
            <section class="memory-summary"><div><span class="metric-icon cyan"><Database :size="18" /></span><p><span>{{ copy.totalRecords }}</span><strong>{{ memoryState.records.length.toLocaleString() }}</strong></p></div><div><span class="metric-icon violet"><HardDrive :size="18" /></span><p><span>{{ copy.databaseSize }}</span><strong>{{ memoryState.status.database_size_bytes == null ? '—' : formatBytes(memoryState.status.database_size_bytes) }}</strong></p></div><div><span class="metric-icon green"><Network :size="18" /></span><p><span>{{ copy.activeConnections }}</span><strong>{{ memoryState.status.connection_count ?? '—' }}</strong></p></div></section>
            <section class="panel data-panel">
              <div class="toolbar"><div class="inline-search"><Search :size="15" /><input v-model="memorySearch" :placeholder="copy.searchMemory" @keyup.enter="loadMemoryCore" /></div><button class="button tertiary" @click="loadMemoryCore"><Search :size="15" />{{ copy.searchMemory }}</button></div>
              <div class="toolbar"><div class="inline-search"><Plus :size="15" /><input v-model="newMemoryContent" :placeholder="copy.memoryContent" @keyup.enter="saveSystemMemory" /></div><button class="button primary" :disabled="memoryBusy || memoryState.status.state !== 'READY' || !newMemoryContent.trim()" @click="saveSystemMemory"><Plus :size="15" />{{ copy.saveMemory }}</button></div>
              <div v-if="memoryRows.length" class="memory-list"><div v-for="memory in memoryRows" :key="memory.id" class="memory-row"><span class="memory-type">{{ memory.type }}</span><div><strong>{{ memory.excerpt }}</strong><span>{{ memory.scope }} · {{ copy.updated }} {{ memory.updated }}</span></div><span class="priority" :class="memory.priorityClass">{{ memory.priority }}</span></div></div>
              <div v-else class="environment-empty"><Database :size="22" /><p>{{ copy.noMemoryRecords }}</p></div>
            </section>
          </template>
        </template>

        <template v-else-if="activePage === 'Environment Explorer'">
          <section v-if="environmentState.kind === 'loading' || environmentState.kind === 'idle'" class="panel environment-message">
            <span class="environment-pulse"><Search :size="25" /></span>
            <h2>{{ copy.scanningEnvironment }}</h2>
          </section>

          <section v-else-if="environmentState.kind === 'desktop_required'" class="panel environment-message">
            <span class="feature-icon"><Network :size="25" /></span>
            <h2>{{ copy.desktopRequired }}</h2>
            <p>{{ copy.desktopRequiredDescription }}</p>
            <div class="environment-fact"><ShieldCheck :size="16" /><span>{{ copy.openDesktopHint }}</span></div>
          </section>

          <section v-else-if="environmentState.kind === 'error'" class="panel environment-message error-state">
            <span class="feature-icon"><Activity :size="25" /></span>
            <h2>{{ copy.scanFailed }}</h2>
            <p>{{ environmentState.message }}</p>
            <button class="button primary" @click="loadEnvironment(true)"><RefreshCw :size="15" />{{ copy.retry }}</button>
            <details><summary>{{ copy.technicalDetails }}</summary><code>{{ environmentState.technical }}</code></details>
          </section>

          <template v-else-if="environmentState.kind === 'ready'">
            <section class="environment-summary">
              <article class="panel"><span class="metric-icon blue"><AppWindow :size="18" /></span><p><span>{{ copy.detectedAssets }}</span><strong>{{ environmentState.snapshot.assets.length }}</strong></p></article>
              <article class="panel"><span class="metric-icon cyan"><Blocks :size="18" /></span><p><span>{{ copy.providedCapabilities }}</span><strong>{{ environmentCapabilityCount }}</strong></p></article>
              <article class="panel"><span class="metric-icon violet"><Route :size="18" /></span><p><span>{{ copy.scannedRoots }}</span><strong>{{ environmentState.snapshot.roots_scanned.length }}</strong></p></article>
              <article class="panel"><span class="metric-icon green"><Check :size="18" /></span><p><span>{{ copy.lastObserved }}</span><strong class="observed-time">{{ observedLabel(environmentState.snapshot.scanned_at) }}</strong></p></article>
            </section>
            <section class="panel environment-panel">
              <div class="toolbar"><div class="inline-search environment-search"><Search :size="16" /><input v-model="environmentSearch" :placeholder="copy.searchEnvironment" /></div><span class="verified-label"><ShieldCheck :size="15" />{{ copy.verifiedByCore }}</span></div>
              <div v-if="filteredEnvironmentAssets.length" class="environment-grid">
                <article v-for="asset in filteredEnvironmentAssets" :key="asset.id" class="environment-card">
                  <div class="environment-card-head"><span class="environment-asset-icon"><TerminalSquare v-if="asset.kind === 'executable'" :size="18" /><AppWindow v-else-if="asset.kind === 'application'" :size="18" /><Blocks v-else :size="18" /></span><div><h2>{{ asset.name }}</h2><span>{{ categoryLabel(asset.category) }} · {{ kindLabel(asset.kind) }}</span></div><i class="ready-dot"></i></div>
                  <dl><div><dt>{{ copy.location }}</dt><dd class="mono">{{ asset.location }}</dd></div><div><dt>{{ copy.capability }}</dt><dd><span v-for="capability in asset.capabilities" :key="capability" class="capability-tag">{{ capability }}</span></dd></div></dl>
                  <footer><span><Check :size="13" />{{ copy.verifiedByCore }}</span><time>{{ observedLabel(asset.observed_at) }}</time></footer>
                </article>
              </div>
              <div v-else class="environment-empty"><Search :size="22" /><p>{{ copy.noEnvironmentAssets }}</p></div>
            </section>
          </template>
        </template>

        <template v-else-if="activePage === 'AI Test Console'">
          <section class="console-layout">
            <article class="panel console-panel">
              <div class="console-toolbar">
                <span><FlaskConical :size="16" />{{ copy.diagnosticRun }}</span>
                <div><span class="mode-tag">{{ copy.localOnlyExecution }}</span>
                  <select v-model="selectedLocalModel" class="small-select" :disabled="localAiState.kind !== 'ready'">
                    <option value="">{{ copy.selectLocalModel }}</option>
                    <option v-for="model in localModels" :key="model.reference.model_id" :value="model.reference.model_id">{{ model.display_name }}</option>
                  </select>
                </div>
              </div>
              <div class="console-body">
                <div v-if="localAiState.kind === 'idle' || localAiState.kind === 'loading'" class="console-empty"><CircleGauge :size="28" /><h2>{{ copy.localAiChecking }}</h2></div>
                <div v-else-if="localAiState.kind === 'desktop_required'" class="console-empty"><Bot :size="28" /><h2>{{ copy.desktopRequired }}</h2><p>{{ copy.desktopAiRequired }}</p></div>
                <div v-else-if="localAiState.kind === 'error'" class="console-empty"><Activity :size="28" /><h2>{{ localAiState.message }}</h2><p>{{ localAiState.technical }}</p></div>
                <div v-else-if="localAiState.status.health.state !== 'healthy' || localAiState.status.models.length === 0" class="console-empty"><Activity :size="28" /><h2>{{ localAiState.status.models.length ? copy.localAiUnavailable : copy.noLocalModels }}</h2><p>{{ copy.localAiUnavailableHint }}</p></div>
                <div v-else-if="localResponse" class="local-response"><div class="local-response-heading"><Bot :size="20" /><strong>{{ copy.response }}</strong><span>{{ localResponse.model.model_id }}</span></div><p>{{ localResponse.text }}</p></div>
                <div v-else class="console-empty"><span><Bot :size="28" /></span><h2>{{ copy.testPipeline }}</h2><p>{{ copy.testPipelineDescription }}</p></div>
              </div>
              <div class="prompt-box">
                <textarea v-model="localPrompt" :aria-label="copy.diagnosticPrompt" :placeholder="copy.enterTask" :disabled="localRunPending"></textarea>
                <div><span>{{ copy.localOnlyExecution }}</span><button :disabled="localRunPending || !selectedLocalModel" @click="runLocalTest"><Zap :size="15" fill="currentColor" />{{ localRunPending ? copy.running : copy.runTest }}</button></div>
                <p v-if="localRunError" class="console-error">{{ localRunError }}</p>
              </div>
            </article>
            <aside class="panel inspector">
              <div class="panel-header"><div><h2>{{ copy.runInspector }}</h2><p>{{ copy.contextRoutingDetails }}</p></div></div>
              <div v-if="localResponse" class="run-facts"><span><strong>{{ copy.provider }}</strong>Ollama</span><span><strong>{{ copy.model }}</strong>{{ localResponse.model.model_id }}</span><span><strong>{{ copy.inputTokens }}</strong>{{ localResponse.usage.input_tokens }}</span><span><strong>{{ copy.outputTokens }}</strong>{{ localResponse.usage.output_tokens }}</span><span><strong>{{ copy.execution }}</strong>{{ copy.localOnlyExecution }}</span></div>
              <div v-else class="inspector-empty"><CircleGauge :size="24" /><p>{{ copy.runInspectorEmpty }}</p></div>
            </aside>
          </section>
        </template>

        <template v-else-if="activePage === 'Developer Agent'">
          <section class="developer-shell">
            <article class="panel ard-team-panel">
              <div class="panel-header">
                <div><p class="eyebrow">ARD · Agent Relay Development</p><h2>{{ locale === 'ja' ? 'AIチーム・人事' : 'AI Team & People' }}</h2><p>{{ locale === 'ja' ? 'RoleとModelを分離し、担当者を順番にRelayします。' : 'Separate roles from models and relay work between team members.' }}</p></div>
                <span class="status-badge" :class="ardSession?.state === 'RUNNING' ? 'healthy' : 'neutral'">{{ ardSession?.state ?? (locale === 'ja' ? '未実行' : 'Idle') }}</span>
              </div>
              <div class="ard-toolbar">
                <label>{{ locale === 'ja' ? 'チーム' : 'Team' }}<select v-model="selectedArdTeam" class="small-select" @change="loadArdWorkflows"><option value="">{{ locale === 'ja' ? 'チームを選択' : 'Select team' }}</option><option v-for="team in ardTeams" :key="team.id" :value="team.id">{{ team.name }} · {{ team.members.length }} members</option></select></label>
                <label>Workflow<select v-model="selectedArdWorkflow" class="small-select"><option value="">{{ locale === 'ja' ? 'Workflowを選択' : 'Select workflow' }}</option><option v-for="workflow in ardWorkflows" :key="workflow.id" :value="workflow.id">{{ workflow.name }}</option></select></label>
              </div>
              <div v-if="selectedArdTeam" class="ard-members">
                <div v-for="member in ardTeams.find((team) => team.id === selectedArdTeam)?.members ?? []" :key="member.id" class="ard-member-card">
                  <span class="agent-orb"><Bot :size="15" /></span><div><strong>{{ member.name }}</strong><small>{{ member.role }} · {{ member.brain.kind === 'auto' ? 'Brain: Auto' : `${member.brain.provider_id}/${member.brain.model_id}` }}</small><small v-if="latestBrainResolution(member.id)" class="ard-resolution">{{ locale === 'ja' ? '解決済み' : 'Resolved' }}: {{ latestBrainResolution(member.id)?.provider_id }}/{{ latestBrainResolution(member.id)?.model_id }} · {{ latestBrainResolution(member.id)?.compatibility }} ({{ latestBrainResolution(member.id)?.score }})</small><small v-if="latestBrainResolution(member.id)" :title="latestBrainResolution(member.id)?.reason">{{ latestBrainResolution(member.id)?.reason }}</small><small>{{ member.permission.allowed.join(' · ') }}</small></div>
                </div>
              </div>
              <div v-else class="ard-create-row">
                <input v-model="ardTeamName" :placeholder="locale === 'ja' ? 'チーム名' : 'Team name'" />
                <button class="button secondary" :disabled="developerBusy || !selectedDeveloperWorkspace" @click="createArdPresetTeam"><Plus :size="15" />{{ locale === 'ja' ? 'Auto選定の標準3担当チームを作成' : 'Create Auto-routed 3-member preset' }}</button>
              </div>
              <label class="developer-task-input"><span>{{ locale === 'ja' ? 'ARDの目標' : 'ARD goal' }}</span><textarea v-model="ardGoal" :disabled="ardSession?.state === 'RUNNING'" /></label>
              <div class="developer-actions">
                <span><ShieldCheck :size="15" />{{ locale === 'ja' ? '担当者ごとのTool権限をCoreで強制' : 'Per-member tool permissions enforced by Core' }}</span>
                <button v-if="ardSession && ['RUNNING','PAUSED'].includes(ardSession.state)" class="button secondary" :disabled="developerBusy" @click="toggleArdPause">{{ ardSession.state === 'PAUSED' ? (locale === 'ja' ? '再開' : 'Resume') : (locale === 'ja' ? '一時停止' : 'Pause') }}</button>
                <button v-if="ardSession && ['RUNNING','PAUSED','WAITING_APPROVAL'].includes(ardSession.state)" class="button danger" :disabled="developerBusy" @click="stopArdRelay"><X :size="15" />{{ locale === 'ja' ? '停止' : 'Stop' }}</button>
                <button v-else class="button primary" :disabled="developerBusy || !selectedArdWorkflow || !ardGoal.trim()" @click="runArdRelay"><Route :size="15" />Start ARD</button>
              </div>
              <div v-if="ardSession" class="ard-activity-strip">
                <div v-for="event in ardSession.activity.slice(-6)" :key="event.sequence"><time>{{ new Date(event.occurred_at).toLocaleTimeString(locale === 'ja' ? 'ja-JP' : 'en-US') }}</time><span>{{ event.message }}</span></div>
              </div>
              <div v-if="ardSession?.active_model" class="ard-runtime-status">
                <span><strong>{{ locale === 'ja' ? '現在のモデル' : 'Current model' }}</strong>{{ ardSession.active_model }}</span>
                <span><strong>Runtime</strong>{{ ardSession.active_runtime ?? '—' }}</span>
                <span><strong>{{ locale === 'ja' ? 'Rotation' : 'Rotation' }}</strong>{{ ardSession.active_rotation?.status ?? ardSession.model_rotations.at(-1)?.status ?? '—' }}</span>
              </div>
            </article>
            <article class="panel developer-control">
              <div class="panel-header">
                <div><p class="eyebrow">Vertex Developer Agent · Phase 1</p><h2>{{ locale === 'ja' ? '安全な自己開発ワークスペース' : 'Safe self-development workspace' }}</h2></div>
                <span class="status-badge" :class="developerTask?.state === 'COMPLETED' ? 'healthy' : 'neutral'">{{ developerTask ? developerStateLabel(developerTask.state) : (locale === 'ja' ? '未実行' : 'Idle') }}</span>
              </div>
              <div v-if="developerError === 'desktop_required'" class="console-empty compact"><Bot :size="28" /><h2>{{ copy.desktopRequired }}</h2><p>{{ copy.desktopRequiredDescription }}</p></div>
              <template v-else>
                <div class="developer-form-grid">
                  <label>{{ locale === 'ja' ? 'プロジェクト' : 'Project' }}<select v-model="selectedDeveloperWorkspace" class="small-select"><option value="">{{ locale === 'ja' ? 'Workspaceを選択' : 'Select workspace' }}</option><option v-for="workspace in developerWorkspaces" :key="workspace.id" :value="workspace.id">{{ workspace.name }} — {{ workspace.root }}</option></select></label>
                  <label>{{ locale === 'ja' ? 'モデル' : 'Model' }}<select v-model="selectedLocalModel" class="small-select"><option value="">{{ copy.selectLocalModel }}</option><option v-for="model in localModels" :key="model.reference.model_id" :value="model.reference.model_id">{{ model.display_name }}</option></select></label>
                </div>
                <div class="developer-mode" role="radiogroup" :aria-label="locale === 'ja' ? '実行モード' : 'Execution mode'">
                  <button v-for="mode in developerModes" :key="mode" :class="{ active: developerMode === mode }" @click="developerMode = mode">{{ mode }}</button>
                </div>
                <label class="developer-task-input"><span>{{ locale === 'ja' ? '開発要求' : 'Development request' }}</span><textarea v-model="developerRequest" :disabled="developerTaskActive" /></label>
                <div class="developer-actions">
                  <span><ShieldCheck :size="15" />{{ locale === 'ja' ? 'Workspace外書込み・Secret読込・危険コマンドを拒否' : 'Workspace escape, secret reads, and dangerous commands are denied' }}</span>
                  <button v-if="developerTaskActive" class="button danger" :disabled="developerBusy" @click="stopDeveloperAgent"><X :size="15" />{{ locale === 'ja' ? 'キャンセル' : 'Cancel' }}</button>
                  <button v-else class="button primary" :disabled="developerBusy || !selectedDeveloperWorkspace || !selectedLocalModel || !developerRequest.trim()" @click="runDeveloperAgent"><Zap :size="15" />{{ developerBusy ? copy.running : (locale === 'ja' ? '実行' : 'Run') }}</button>
                </div>
                <p v-if="developerError" class="console-error">{{ developerError }}</p>
              </template>
            </article>

            <article class="panel developer-activity">
              <div class="panel-header"><div><h2>{{ locale === 'ja' ? 'エージェント活動' : 'Agent Activity' }}</h2><p>{{ locale === 'ja' ? '計画・Tool・Build/Testをリアルタイム表示' : 'Live plan, tools, build, and test activity' }}</p></div><span v-if="developerTask">{{ developerTask.steps_completed }} steps · {{ developerTask.tool_calls }} tools</span></div>
              <div v-if="!developerTask" class="console-empty compact"><Code2 :size="28" /><h2>{{ locale === 'ja' ? 'タスクを入力してください' : 'Enter a task' }}</h2><p>{{ locale === 'ja' ? '最初はREAD ONLYでRepository構造を確認できます。' : 'Start with READ ONLY to inspect repository structure.' }}</p></div>
              <div v-else class="activity-timeline">
                <div v-for="activity in developerTask.activities" :key="activity.sequence" class="activity-row"><time>{{ new Date(activity.occurred_at).toLocaleTimeString(locale === 'ja' ? 'ja-JP' : 'en-US') }}</time><span class="activity-dot" :class="activity.risk.toLowerCase()"></span><div><strong>{{ activity.message }}</strong><small v-if="activity.detail">{{ activity.detail }}</small></div></div>
              </div>
            </article>

            <aside class="panel developer-inspector">
              <div class="panel-header"><div><h2>{{ locale === 'ja' ? 'タスク状態' : 'Task Status' }}</h2><p>{{ developerTask?.model ?? '—' }}</p></div></div>
              <div class="developer-facts">
                <span><small>{{ locale === 'ja' ? '状態' : 'State' }}</small><strong>{{ developerTask ? developerStateLabel(developerTask.state) : '—' }}</strong></span>
                <span><small>{{ locale === 'ja' ? '変更ファイル' : 'Files changed' }}</small><strong>{{ developerTask?.files_changed.length ?? 0 }}</strong></span>
                <span><small>Build / Test</small><strong :class="{ pass: developerValidation === 'PASS' }">{{ developerValidation }}</strong></span>
                <span><small>{{ locale === 'ja' ? 'リスク' : 'Risk' }}</small><strong>{{ developerTask ? developerRiskLabel(developerTask.risk) : '—' }}</strong></span>
                <span><small>{{ locale === 'ja' ? '信頼度' : 'Confidence' }}</small><strong>{{ Math.round((developerTask?.confidence ?? 0) * 100) }}%</strong></span>
                <span><small>{{ locale === 'ja' ? '失敗試行' : 'Failed attempts' }}</small><strong>{{ developerTask?.failed_attempts ?? 0 }}</strong></span>
              </div>
              <p v-if="developerTask?.result_summary" class="developer-summary">{{ developerTask.result_summary }}</p>
              <button v-if="developerTask?.files_changed.length" class="button secondary" :disabled="developerTaskActive || developerBusy" @click="rollbackDeveloperChanges">{{ locale === 'ja' ? '変更をロールバック' : 'Rollback changes' }}</button>
            </aside>

            <article class="panel developer-plan">
              <div class="panel-header"><div><h2>{{ locale === 'ja' ? '実装計画' : 'Implementation Plan' }}</h2><p v-if="latestDeveloperPlan">v{{ latestDeveloperPlan.version }} · {{ latestDeveloperPlan.reason }}</p></div></div>
              <ol v-if="latestDeveloperPlan"><li v-for="step in latestDeveloperPlan.steps" :key="step.id" :class="step.state"><span>{{ step.id }}</span>{{ step.description }}</li></ol><p v-else class="muted-copy">{{ locale === 'ja' ? '計画はAgent開始後に生成されます。' : 'The plan appears after the agent starts.' }}</p>
            </article>

            <article class="panel developer-diff">
              <div class="panel-header"><div><h2>Diff</h2><p>{{ locale === 'ja' ? '内部Checkpointとの差分' : 'Changes from internal checkpoint' }}</p></div></div>
              <pre>{{ developerTask?.unified_diff || (locale === 'ja' ? '変更はありません' : 'No changes') }}</pre>
            </article>

            <article class="panel developer-terminal">
              <div class="panel-header"><div><h2>Terminal</h2><p>{{ locale === 'ja' ? '実行コマンド・標準出力・終了コード' : 'Commands, output, and exit codes' }}</p></div></div>
              <details v-for="command in developerTask?.commands ?? []" :key="command.id"><summary><code>&gt; {{ command.executable }} {{ command.args.join(' ') }}</code><span :class="command.status === 'COMPLETED' ? 'pass' : 'fail'">{{ command.status }} · {{ command.exit_code ?? '—' }}</span></summary><pre>{{ command.stdout }}{{ command.stderr ? `\n${command.stderr}` : '' }}</pre></details><p v-if="!developerTask?.commands.length" class="muted-copy">{{ locale === 'ja' ? 'コマンド実行はまだありません。' : 'No commands have run.' }}</p>
            </article>

            <article class="panel developer-register">
              <div class="panel-header"><div><h2>{{ locale === 'ja' ? 'Workspace登録' : 'Register workspace' }}</h2><p>{{ locale === 'ja' ? '複数Projectを安全境界ごとに管理' : 'Manage multiple projects with separate safety boundaries' }}</p></div></div>
              <div><input v-model="developerWorkspaceName" :placeholder="locale === 'ja' ? 'プロジェクト名' : 'Project name'" /><input v-model="developerWorkspaceRoot" :placeholder="locale === 'ja' ? '絶対パス' : 'Absolute path'" /><button class="button secondary" :disabled="developerBusy" @click="addDeveloperWorkspace"><Plus :size="15" />{{ locale === 'ja' ? '登録' : 'Register' }}</button></div>
            </article>
          </section>
        </template>

        <template v-else-if="activePage === 'Settings'">
          <section class="settings-grid">
            <article class="panel settings-card">
              <div class="settings-heading"><span class="feature-icon"><Languages :size="24" /></span><div><p class="eyebrow">{{ copy.language }}</p><h2>{{ copy.languageTitle }}</h2><p>{{ copy.languageDescription }}</p></div></div>
              <div class="language-options" role="radiogroup" :aria-label="copy.languageTitle">
                <button role="radio" :aria-checked="locale === 'ja'" :class="{ active: locale === 'ja' }" @click="setLocale('ja')"><span>日</span><div><strong>{{ copy.japanese }}</strong><small>{{ copy.japaneseDetail }}</small></div><Check v-if="locale === 'ja'" :size="18" /></button>
                <button role="radio" :aria-checked="locale === 'en'" :class="{ active: locale === 'en' }" @click="setLocale('en')"><span>EN</span><div><strong>{{ copy.english }}</strong><small>{{ copy.englishDetail }}</small></div><Check v-if="locale === 'en'" :size="18" /></button>
              </div>
              <p class="settings-note"><ShieldCheck :size="15" />{{ copy.savedOnDevice }}</p>
            </article>
            <article class="panel settings-scope"><p class="eyebrow">{{ copy.settingsScope }}</p><h2>{{ copy.settingsScope }}</h2><p>{{ copy.settingsScopeDescription }}</p></article>
          </section>
        </template>

        <template v-else>
          <section class="placeholder-grid"><article class="panel feature-intro"><span class="feature-icon"><component :is="navItems.find(item => item.key === activePage)?.icon" :size="24" /></span><div><p class="eyebrow">{{ copy.managementBoundary }}</p><h2>{{ activePageLabel }}</h2><p>{{ pageDescription }}{{ locale === 'ja' ? '。' : '. ' }}{{ copy.placeholderSuffix }}</p></div><button class="button primary">{{ copy.openConfiguration }}<ChevronRight :size="15" /></button></article><article class="panel readiness"><h2>{{ copy.integrationReadiness }}</h2><div><span><Check :size="14" />{{ copy.uiBoundaryDefined }}</span><span><Check :size="14" />{{ copy.coreHeadless }}</span><span><Check :size="14" />{{ copy.noDirectDatabase }}</span></div></article></section>
        </template>
      </div>
    </main>

    <div v-if="providerDialog" class="modal-layer" @click.self="providerDialog = false">
      <div class="modal" role="dialog" aria-modal="true" aria-labelledby="provider-title"><div class="modal-head"><div><span class="modal-icon"><Cloud :size="20" /></span><div><p>{{ copy.providerConnection }}</p><h2 id="provider-title">{{ providerTitle() }}</h2></div></div><button class="icon-button" @click="providerDialog = false"><X :size="18" /></button></div><p class="modal-copy">{{ copy.providerSecretCopy }}</p><label class="field-label">{{ copy.apiKey }}<div class="secret-input"><KeyRound :size="16" /><input v-model="apiKey" type="password" autocomplete="off" :placeholder="copy.enterApiKey" /></div></label><div class="security-note"><ShieldCheck :size="17" /><span><strong>{{ copy.protectedByOs }}</strong>{{ copy.plaintextDisabled }}</span></div><div class="modal-actions"><button class="button secondary" @click="providerDialog = false">{{ copy.cancel }}</button><button class="button primary" @click="connectProvider">{{ copy.connectDiscover }}</button></div></div>
    </div>

    <div v-if="commandOpen" class="modal-layer command-layer" @click.self="commandOpen = false">
      <div class="command-modal"><div class="command-input"><Search :size="18" /><input autofocus :placeholder="copy.jumpPage" /><kbd>ESC</kbd></div><div class="command-results"><p>{{ copy.navigation }}</p><button v-for="item in navItems" :key="item.key" @click="navigate(item.key)"><component :is="item.icon" :size="16" />{{ navLabel(item.key) }}<span>{{ copy.go }}</span></button></div></div>
    </div>

    <transition name="toast"><div v-if="toast" class="toast"><Check :size="15" />{{ toast }}</div></transition>
  </div>
</template>

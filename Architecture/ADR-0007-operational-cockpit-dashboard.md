# ADR-0007: DashboardをOperational Cockpitへ再設計

- 状態: 採用
- 日付: 2026-08-12

## 背景

従来のDashboardは統計カードと履歴グラフを中心としており、起動直後に「現在何が動いているか」「問題があるか」「何を操作すべきか」を判断しにくかった。Vertex AIは一般的な分析ダッシュボードではなく、ローカル・クラウドAI、ランタイム、モデル、ストレージ、エージェントを統合管理するAI Environment Operating Consoleを目指す。

## 決定

1. DashboardのPriority 1をOperational Center、Action Center、Running Tasks、Runtime Status、Storage / VRAMとする。
2. 既存の履歴グラフは削除せず、補助情報として画面占有率を縮小する。
3. Ollama状態、導入モデル、ロード済みモデル、VRAM割当、モデル保存先、ドライブ容量、ダウンロードジョブは既存Tauri IPCの実データを使用する。
4. CPU、RAM、GPU、PostgreSQL、Knowledge Core、Codex、Reviewer、Judge、Councilはバックエンド契約が存在するまで「計測API接続待ち」と明示する。
5. 未接続項目へ架空の数値、正常状態、実行中タスクを表示しない。
6. Action Centerは確認済みランタイム停止、モデル未導入、空き容量不足、モデル取得失敗から優先項目を決定し、既存の安全な詳細画面へ誘導する。
7. モデル移動、削除、Smart Fixなど未実装の破壊的操作ボタンは表示しない。
8. 履歴グラフと最近のルーティング判断に実バックエンドが接続されるまではプレビューデータ表示を付ける。
9. 日本語を標準とし、追加表示はすべて英語切替へ対応する。

## UI構造

- 上部KPI: 利用可能モデル、ロード済みモデル、実行中ジョブ、システム健全性
- 中央主領域: Operational CenterとAction Center
- 第2領域: Resource Monitor、Active Models、AI Activity
- 補助領域: System Health、縮小した処理履歴、最近のルーティング判断

## 今後

- System Health診断契約と健全性スコアのCore計算
- Windows Performance Counterまたは安全なクロスプラットフォーム指標取得
- PostgreSQL / Knowledge Coreのヘルス・統計IPC
- AI Agent Activityイベントストリーム
- 承認、バックアップ、検証、ロールバックを備えたSmart Fix接続

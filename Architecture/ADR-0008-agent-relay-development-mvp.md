# ADR-0008: Agent Relay Development MVP

- 状態: 採用・MVP Core実装
- 日付: 2026-08-14

## 背景

Vertex Developer Agentは単一モデルによるWorkspace探索・編集・Terminal実行を提供するが、役割を持つ複数のAI担当者が単一GPU上で交代しながら開発する状態管理は存在しなかった。RoleとModelを同一概念にせず、チャット全文ではなく構造化情報を引き継ぐ必要がある。

## 決定

1. ARDを`vertex-ai-developer`の上位Orchestrationとして追加し、既存Workspace/File/Terminal/Agent Loopを再利用する。
2. Teamは任意数のMemberを持ち、各MemberへRole、Brain（Autoまたは明示Model）、Workspace、Hard Permission、責任、禁止事項、上位担当、Handoff先を割り当てる。
3. ModelへのSoft PolicyとTool LayerのHard Permissionを分離する。Reviewer等のRead Only担当者はWrite Tool自体を許可されない。
4. Handoffは`task_result`、判断、読込・変更ファイル、Test、既知問題、未解決事項、次Action、Confidenceを持つTyped Dataとする。
5. Workflowは任意Stageで表現し、標準RelayではArchitect→Developer→Reviewerを提供する。ReviewerのReworkはDeveloperへ戻し、Stage単位のRetry上限で無限ループを防ぐ。
6. SessionはRunning、Paused、Waiting Approval、Completed、Failed、Cancelledを持ち、ユーザー介入を構造化して現在担当へ伝播する。
7. 稼働中に異常終了したSessionは次回起動時にCompleted扱いせずPausedとして復元する。
8. Model Rotationは担当交代ごとにfrom/to、同一Model再利用、Auto Router要否を記録する。同一Modelの場合は不要なUnload/Reloadを避ける。
9. ARD StateはTauriアプリ専用領域へ原子的JSON保存する。Project Brain/PostgreSQLへの統合は次の永続化段階で行う。

## 安全性

- Repository内容はUntrusted Project DataとしてRole Policyへ明示する。
- 権限判定はPromptだけに依存せず、`ToolCall`ごとにCapabilityとRisk上限をCoreで検証する。
- Memberは割り当てられたWorkspace以外のProjectへ権限を継承しない。
- Git書込み、Delete、Networkは明示Capabilityがない限り拒否する。
- Retry上限またはHandoff先欠落時はHuman Decision Requiredへ遷移する。

## MVP境界

実装済みはTeam/Role/Permission/Handoff/Workflow/差し戻し/Pause/Resume/Recovery/Rotation状態/Tauri API/最小UIである。各Stageから既存Developer Agent Taskを自動起動し、Model Runtimeを実際にLoad/Unloadし、Project Brainへ保存する実行Bridgeは次段階とする。

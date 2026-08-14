# Vertex AI 0.1.3 — PostgreSQL Private Runtime 実装報告

## 結果

PostgreSQL 18.4 Windows x64 RuntimeをVertex AIのNSISインストーラーへ同梱し、外部PostgreSQLを必要としないVertex Memory Coreの初回自動構築、起動、停止、再起動、診断、Schema Migration、Memory永続化を実装した。

生成物：`InstallPackage/Vertex-AI-0.1.3-x64-setup.exe`

SHA-256：`f824f5235c78f0acada0441fc2fc6c80afea944895962927b8ac7765669034c8`

Installerサイズ：34,479,317 bytes（約32.88 MiB）

## Architecture

```text
Vue Management UI
        ↓ Tauri IPC
Vertex AI Core / MemoryService
        ↓
ManagedPostgresRuntime
        ↓
PostgresMemoryRepository
        ↓
Vertex Managed PostgreSQL 18.4
```

PostgreSQLが停止または初期化に失敗しても、CoreはMemoryなしの縮退状態で起動し、UI、Ollama、Cloud Provider、Environment Managerを継続利用できる。

## Runtime

- Version: PostgreSQL 18.4（EDB Windows x86-64 binaries 18.4-2）
- Source archive SHA-256: `02e239529ed7833d169f98d915d3feffe0813264b08b3ae353e78e8b9c97e1a6`
- Source: `https://get.enterprisedb.com/postgresql/postgresql-18.4-2-windows-x64-binaries.zip`
- 同梱サイズ: 148,122,748 bytes、1,588 files
- 同梱対象: `bin`、`lib`、`share`、ライセンス通知
- 除外対象: pgAdmin、StackBuilder、開発用header、documentation

## 配置

- Installer内Runtime: `$INSTDIR/runtime/postgresql/18.4/pgsql`
- 開発Runtime: `ProgramSource/apps/management-ui/src-tauri/runtime/postgresql/18.4/pgsql`
- Data: Tauri `app_data_dir/Memory/PostgreSQL`
- Cluster: `app_data_dir/Memory/PostgreSQL/Cluster`
- Runtime Manifest: `app_data_dir/Memory/PostgreSQL/runtime-v1.json`
- Log: `app_data_dir/Memory/PostgreSQL/Logs/postgresql.log`

Runtime更新時にDataディレクトリを上書きしない。アンインストーラーではMemory保持を既定選択とし、完全削除には二段階確認を要求する。

## Credential方式

- Windows Credential Managerを使用
- Bootstrap Role: `vertex_ai_bootstrap`
- Application Role: `vertex_ai_app`
- Database: `vertex_ai`
- 32-byte OS乱数を64桁hex credentialへ変換
- PasswordをJSON、ソース、ログ、接続URLへ保存しない
- SQLx `PgConnectOptions`で接続し、URL直列化を回避

## Network / Port方式

- `listen_addresses = '127.0.0.1'`
- LAN公開なし
- 初回はOSから利用可能な動的portを取得
- 選択portをRuntime Manifestへ永続化
- 再起動時に別processとの競合を検出した場合は、新しいloopback portを選択して永続化
- 既存の外部PostgreSQLや5432固定に依存しない

## Migration方式

- 既存`vertex-ai-memory/migrations/0001_memory_core.sql`をSQLx Migratorで再利用
- Runtime起動、DB/Role作成後にMigrationを実施
- Migration失敗時はMemoryをReady扱いにしない
- Application Version、Runtime Version、Schema Versionを分離
- PostgreSQL major version不一致時は自動差替えせず`REPAIR_REQUIRED`とし、`pg_upgrade`前提で停止

## UI / Health

- Vertex Memory Coreを上位表示
- READY / STOPPED / DEGRADED / ERROR / REPAIR_REQUIREDに対応
- Runtime version、location、data location、host、port、database size、connection count、schema versionを表示
- 起動、停止、再起動、診断をIPC経由で実行
- System scopeのMemory保存と全文検索を実データへ接続
- UIからPostgreSQLを直接操作しない

## Test結果

- `cargo test --workspace --no-fail-fast`: 46 passed / 0 failed / 3 ignored
- Managed PostgreSQL実Runtime統合テスト: 1 passed / 0 failed（14.29秒）
  - Fresh Cluster作成
  - 専用Role / Database作成
  - Schema Migration
  - Memory保存
  - Runtime停止 / 再起動
  - 再起動後のMemory永続性
- `cargo clippy --workspace --all-targets -- -D warnings`: warning 0
- `vue-tsc -b`: success
- Vite production build: success
- Rust release build: success
- Tauri NSIS build: success
- NSIS内PostgreSQL `bin/lib/share` File entry確認: success
- SHA-256台帳との再計算照合: success

## Known Issues

1. Backup Service境界と状態項目は追加済みだが、`pg_dump`による自動Backup/Restoreは未実装。
2. Environment Doctorは構造化診断まで。Smart Fixによる自動修復、rollbackは未実装。
3. 初回Cluster構築中はTauri setupが完了を待つため、段階別Initial Setup画面は未実装。
4. 別の完全なclean Windows VMでのSetup/Uninstall試験は未実施。外部PostgreSQLなしの一時領域統合試験で代替確認済み。
5. Installerのコード署名は未実施。
6. pgvectorは現在のRepositoryに必須でないため同梱していない。

## 次に必要な作業

1. Clean Windows 10/11 VMでInstall、Upgrade、Reinstall、Uninstall保持/削除を検証する。
2. `pg_dump` / `pg_restore`を使うBackup Serviceを実装する。
3. Initial Setupの段階別進捗UIを追加する。
4. Disk full、credential消失、runtime破損、migration failureのFault Injection testを追加する。
5. コード署名証明書でSetup.exeへ署名する。

Vertex AI 0.1.3 テスト配布パッケージ
====================================

対象OS: Windows 10 22H2以降 / Windows 11（x64）
最新インストーラー: Vertex-AI-0.1.3-x64-setup.exe

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

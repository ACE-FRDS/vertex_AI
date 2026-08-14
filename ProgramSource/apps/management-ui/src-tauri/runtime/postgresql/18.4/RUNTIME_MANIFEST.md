# Vertex Memory Core — PostgreSQL Runtime

- Runtime: PostgreSQL 18.4 (EDB Windows x86-64 binaries, installer build 18.4-2)
- Source: https://get.enterprisedb.com/postgresql/postgresql-18.4-2-windows-x64-binaries.zip
- Source archive SHA-256: `02e239529ed7833d169f98d915d3feffe0813264b08b3ae353e78e8b9c97e1a6`
- Bundled directories: `bin`, `lib`, `share`
- Bundled notices: server and command-line third-party license files

pgAdmin, StackBuilder, development headers, and documentation are intentionally
excluded because Vertex AI exposes PostgreSQL only as a private managed runtime.
The durable cluster is never stored under this immutable runtime directory.

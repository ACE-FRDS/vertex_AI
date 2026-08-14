# ADR-006: Transport-Agnostic Core

Status: Accepted and implemented

Core accepts domain `Command` values and returns domain responses/errors. HTTP, IPC, named pipes, SDKs, and gRPC are adapters and cannot define Core behavior.


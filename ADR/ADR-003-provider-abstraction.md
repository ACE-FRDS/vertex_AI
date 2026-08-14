# ADR-003: Provider Abstraction

Status: Accepted and implemented

All LLM integrations implement the provider-neutral `ModelProvider` interface. Provider-specific authentication and DTOs stay within adapters; registries expose common model metadata to Core.


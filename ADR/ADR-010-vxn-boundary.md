# ADR-010: VXN Boundary

Status: Accepted for Phase 12

Keep VXN generation, validation, permission evaluation, and runtime as separate interfaces. Generated VXN is untrusted data and cannot execute before validation and capability checks.


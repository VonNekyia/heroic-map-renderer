---
title: "0024: Vulkan zuerst, ohne GL"
description: In welcher Reihenfolge der Renderer Adapter und Backends wählt und warum GL nicht dabei ist.
status: gilt
date: 2026-09-26
issues: [11]
code:
  - renderer/src/render/gpu.rs
  - renderer/Cargo.toml
---

# 0024: Vulkan zuerst, ohne GL

## Anlass

wgpu bietet mehrere Backends, und ein Rechner hat oft mehrere Adapter:
eigenständige Karte, Onboard-Grafik, Software.

## Entscheidung

Eine echte Karte vor jedem Software-Adapter, Vulkan vor DX12, eine
eigenständige Karte vor der Onboard-Grafik (`rang`). `--gpu auto` nimmt nur
echte Karten, `--gpu on` auch Software-Adapter. GL baut wgpu nicht mit.
`WGPU_BACKEND` wählt die Backends, `WGPU_ADAPTER_NAME` einen Adapter nach
einem Teil seines Namens. Vulkan ist auf Windows und Linux derselbe
Treiberweg, unter Linux mit Mesa oft der einzige. DX12 bleibt für eine alte
Onboard-Grafik ohne Vulkan-Treiber und für WARP in der Windows-CI. Siehe
[Grafikkarte](../benutzung/grafikkarte.md), „Adapter und Backends“.

## Verworfene Alternativen

- **GL.** Der Weg lief nirgends in der CI; entschieden in der zweiten
  Prüfung von #11.

## Folgen

- Eine Karte nur mit GL-Treiber zeichnet auf der CPU, dasselbe Bild.
- In der CI laufen die GPU-Tests auf lavapipe (Vulkan) unter Ubuntu und auf
  WARP (DX12) unter Windows, derselbe Shader-Weg wie auf einer Karte.

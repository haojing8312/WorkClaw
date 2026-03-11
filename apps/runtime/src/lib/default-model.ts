import type { ModelConfig } from "../types";

export function getDefaultModel(models: ModelConfig[]): ModelConfig | null {
  return models.find((item) => item.is_default) ?? models[0] ?? null;
}

export function getDefaultModelId(models: ModelConfig[]): string | null {
  return getDefaultModel(models)?.id ?? null;
}

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RuntimePreferences } from "../types";

interface UseImmersiveTranslationOptions {
  scene?: string;
  batchSize?: number;
}

type ImmersiveTranslationDisplayMode = "translated_only" | "bilingual_inline";

function normalizeDisplayMode(raw: unknown): ImmersiveTranslationDisplayMode {
  return raw === "bilingual_inline" ? "bilingual_inline" : "translated_only";
}

export function useImmersiveTranslation(
  texts: string[],
  options?: UseImmersiveTranslationOptions,
) {
  const [translatedMap, setTranslatedMap] = useState<Record<string, string>>({});
  const [isTranslating, setIsTranslating] = useState(false);
  const [displayMode, setDisplayMode] = useState<ImmersiveTranslationDisplayMode>("translated_only");
  const translatingRef = useRef(false);

  const scene = options?.scene ?? null;
  const batchSize = Math.max(1, Math.min(200, options?.batchSize ?? 80));

  const candidates = useMemo(
    () =>
      Array.from(
        new Set(
          texts
            .map((text) => text?.trim())
            .filter((text): text is string => Boolean(text) && !translatedMap[text!]),
        ),
      ),
    [texts, translatedMap],
  );

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const prefs = await invoke<RuntimePreferences | null>("get_runtime_preferences");
        if (cancelled) return;
        setDisplayMode(normalizeDisplayMode(prefs?.immersive_translation_display));
      } catch {
        if (!cancelled) setDisplayMode("translated_only");
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (candidates.length === 0 || translatingRef.current) return;
    let cancelled = false;
    translatingRef.current = true;
    setIsTranslating(true);
    void (async () => {
      try {
        const limited = candidates.slice(0, batchSize);
        const translated = await invoke<string[]>("translate_texts_with_preferences", {
          texts: limited,
          scene,
        });
        if (cancelled) return;
        const next: Record<string, string> = {};
        for (let i = 0; i < limited.length; i += 1) {
          next[limited[i]] = translated[i] ?? limited[i];
        }
        setTranslatedMap((prev) => ({ ...prev, ...next }));
      } catch {
        // silently fallback to source text
      } finally {
        translatingRef.current = false;
        if (!cancelled) setIsTranslating(false);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [batchSize, candidates, scene]);

  const renderDisplayText = useCallback(
    (sourceText: string) => {
      const translated = translatedMap[sourceText] ?? sourceText;
      if (displayMode === "bilingual_inline" && translated !== sourceText) {
        return `${translated} (${sourceText})`;
      }
      return translated;
    },
    [displayMode, translatedMap],
  );

  return { translatedMap, isTranslating, displayMode, renderDisplayText };
}

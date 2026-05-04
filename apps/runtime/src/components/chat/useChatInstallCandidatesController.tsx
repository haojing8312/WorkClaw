import { useCallback, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import type { Message, StreamItem } from "../../types";
import { useImmersiveTranslation } from "../../hooks/useImmersiveTranslation";
import { ChatInstallCandidatesPanel } from "./ChatInstallCandidatesPanel";
import {
  type ClawhubInstallCandidate,
  extractInstallCandidates,
  extractInstallCandidatesWithContent,
} from "./chatViewHelpers";

export function parseDuplicateSkillName(error: unknown): string | null {
  const message =
    typeof error === "string"
      ? error
      : error instanceof Error
      ? error.message
      : String(error ?? "");
  const prefix = "DUPLICATE_SKILL_NAME:";
  if (!message.includes(prefix)) return null;
  return message.split(prefix)[1]?.trim() || null;
}

export function useChatInstallCandidatesController({
  messages,
  streamItems,
  installedSkillIds,
  onSkillInstalled,
}: {
  messages: Message[];
  streamItems: StreamItem[];
  installedSkillIds: string[];
  onSkillInstalled?: (skillId: string) => Promise<void> | void;
}) {
  const [pendingInstallSkill, setPendingInstallSkill] = useState<ClawhubInstallCandidate | null>(null);
  const [showInstallConfirm, setShowInstallConfirm] = useState(false);
  const [installingSlug, setInstallingSlug] = useState<string | null>(null);
  const [installError, setInstallError] = useState<string | null>(null);
  const installInFlightRef = useRef(false);
  const installedSkillSet = useMemo(() => new Set(installedSkillIds), [installedSkillIds]);
  const candidateTranslationTexts = useMemo(
    () => [
      ...messages.flatMap((m) =>
        extractInstallCandidatesWithContent(m.streamItems, m.content).flatMap((candidate) => [
          candidate.name,
          candidate.description ?? "",
        ]),
      ),
      ...extractInstallCandidates(streamItems).flatMap((candidate) => [
        candidate.name,
        candidate.description ?? "",
      ]),
    ],
    [messages, streamItems],
  );
  const { renderDisplayText: renderCandidateText } = useImmersiveTranslation(
    candidateTranslationTexts,
    {
      scene: "experts-finder",
      batchSize: 40,
    },
  );

  const renderInstallCandidates = useCallback(
    (rawCandidates: unknown[]) => (
      <ChatInstallCandidatesPanel
        candidates={rawCandidates as ClawhubInstallCandidate[]}
        installError={installError}
        installedSkillSet={installedSkillSet}
        installingSlug={installingSlug}
        renderCandidateText={renderCandidateText}
        onInstallRequest={(candidate) => {
          setInstallError(null);
          setPendingInstallSkill(candidate);
          setShowInstallConfirm(true);
        }}
      />
    ),
    [installError, installedSkillSet, installingSlug, renderCandidateText],
  );

  const handleConfirmInstall = useCallback(async () => {
    if (!pendingInstallSkill || installInFlightRef.current) return;
    installInFlightRef.current = true;
    setInstallError(null);
    setInstallingSlug(pendingInstallSkill.slug);
    try {
      const result = await invoke<{ manifest: { id: string } }>("install_clawhub_skill", {
        slug: pendingInstallSkill.slug,
        githubUrl: pendingInstallSkill.githubUrl ?? pendingInstallSkill.sourceUrl ?? null,
      });
      if (result?.manifest?.id) {
        await onSkillInstalled?.(result.manifest.id);
      }
      setShowInstallConfirm(false);
      setPendingInstallSkill(null);
    } catch (e) {
      const duplicateName = parseDuplicateSkillName(e);
      if (duplicateName) {
        setInstallError(`技能名称冲突：已存在「${duplicateName}」，请先重命名后再安装。`);
      } else {
        setInstallError("安装失败，请重试。");
      }
      console.error("安装技能库技能失败:", e);
    } finally {
      installInFlightRef.current = false;
      setInstallingSlug(null);
    }
  }, [onSkillInstalled, pendingInstallSkill]);

  const handleCancelInstallConfirm = useCallback(() => {
    if (installInFlightRef.current) return;
    setShowInstallConfirm(false);
    setPendingInstallSkill(null);
  }, []);

  return {
    parseDuplicateSkillName,
    renderInstallCandidates,
    setInstallError,
    installDialog: {
      open: showInstallConfirm && Boolean(pendingInstallSkill),
      summary: pendingInstallSkill
        ? `是否安装「${renderCandidateText(pendingInstallSkill.name)}」？`
        : "是否安装该技能？",
      impact: pendingInstallSkill ? `slug: ${pendingInstallSkill.slug}` : undefined,
      loading: Boolean(installingSlug),
      onConfirm: handleConfirmInstall,
      onCancel: handleCancelInstallConfirm,
    },
  };
}

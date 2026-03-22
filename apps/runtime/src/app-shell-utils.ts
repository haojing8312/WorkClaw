import type { SessionInfo, SkillManifest } from "./types";

const BUILTIN_GENERAL_SKILL_ID = "builtin-general";

export function extractErrorMessage(error: unknown, fallback: string): string {
  if (typeof error === "string") {
    return error;
  }
  if (error instanceof Error) {
    return error.message || fallback;
  }
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof (error as { message?: unknown }).message === "string"
  ) {
    return (error as { message: string }).message;
  }
  return fallback;
}

export function getDefaultSkillId(skillList: SkillManifest[]): string | null {
  const builtin = skillList.find((item) => item.id === BUILTIN_GENERAL_SKILL_ID);
  if (builtin) {
    return builtin.id;
  }
  return skillList[0]?.id ?? null;
}

export function getAdjacentSessionId(
  list: SessionInfo[],
  sessionId: string,
): string | null {
  const index = list.findIndex((item) => item.id === sessionId);
  if (index < 0) {
    return null;
  }
  return list[index + 1]?.id ?? list[index - 1]?.id ?? null;
}

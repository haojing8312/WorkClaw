import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import type { Dispatch, SetStateAction } from "react";
import { reportFrontendDiagnostic } from "../diagnostics";
import type { ImRoleDispatchRequest, Message } from "../types";

type ImBridgeSessionContext = {
  threadId: string;
  sourceChannel: string;
  primaryRoleName: string;
  roleName: string;
  streamBuffer: string;
  streamSentCount: number;
  waitingForAnswer: boolean;
  streamFlushTimer: ReturnType<typeof setTimeout> | null;
  fallbackReplyTimer: ReturnType<typeof setTimeout> | null;
  lastStreamFlushAt: number;
  streamFlushInFlight: boolean;
};

function formatFeishuRoleMessage(roleName: string, text: string): string {
  const safeRole = (roleName || "").trim() || "智能体员工";
  const safeText = (text || "").trim();
  if (!safeText) return "";
  return `[${safeRole}] ${safeText}`;
}

function extractErrorMessage(error: unknown, fallback: string): string {
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

export function useImBridgeIntegration(options: {
  setImManagedSessionIds: Dispatch<SetStateAction<string[]>>;
}) {
  const { setImManagedSessionIds } = options;

  useEffect(() => {
    if (
      typeof window === "undefined" ||
      !(window as unknown as { __TAURI_INTERNALS__?: { transformCallback?: unknown } })
        .__TAURI_INTERNALS__?.transformCallback
    ) {
      return;
    }
    const seen = new Set<string>();
    const sessionContexts = new Map<string, ImBridgeSessionContext>();
    const feishuRetryTimers = new Map<string, ReturnType<typeof setTimeout>>();
    const outboundImDedup = new Map<string, number>();
    const STREAM_CHUNK_SIZE = 120;
    const STREAM_FLUSH_INTERVAL_MS = 1200;
    const FEISHU_RETRY_DELAYS_MS = [1000, 3000, 8000];
    const FEISHU_MAX_ATTEMPTS = FEISHU_RETRY_DELAYS_MS.length + 1;
    const IM_OUTBOUND_DEDUP_WINDOW_MS = 2500;
    const sanitizeInboundPrompt = (raw: string): string =>
      raw
        .replace(/(^|\s)@_[A-Za-z0-9_]+/g, "$1")
        .replace(/(^|\s)@[^\s@]+/g, "$1")
        .replace(/\s+/g, " ")
        .trim();

    const markImManagedSession = (sessionId: string) => {
      setImManagedSessionIds((prev) => {
        if (prev.includes(sessionId)) return prev;
        return [...prev, sessionId];
      });
    };

    const scheduleImStreamFlush = (sessionId: string, delayMs: number) => {
      const ctx = sessionContexts.get(sessionId);
      if (!ctx || ctx.streamFlushTimer) return;
      const safeDelay = Math.max(20, delayMs);
      ctx.streamFlushTimer = setTimeout(() => {
        const current = sessionContexts.get(sessionId);
        if (!current) return;
        current.streamFlushTimer = null;
        void flushImStream(sessionId);
      }, safeDelay);
    };

    const buildChannelRetryKey = (channel: string, threadId: string, text: string) =>
      `${channel}::${threadId}::${text}`;

    const shouldSuppressOutboundDuplicate = (channel: string, threadId: string, text: string) => {
      const key = buildChannelRetryKey(channel, threadId, text);
      const now = Date.now();
      for (const [entryKey, timestamp] of outboundImDedup.entries()) {
        if (now - timestamp > IM_OUTBOUND_DEDUP_WINDOW_MS) {
          outboundImDedup.delete(entryKey);
        }
      }
      const previous = outboundImDedup.get(key);
      if (typeof previous === "number" && now - previous < IM_OUTBOUND_DEDUP_WINDOW_MS) {
        return true;
      }
      outboundImDedup.set(key, now);
      return false;
    };

    const clearFeishuRetryTimer = (key: string) => {
      const timer = feishuRetryTimers.get(key);
      if (timer) {
        clearTimeout(timer);
      }
      feishuRetryTimers.delete(key);
    };

    const clearImFallbackReplyTimer = (sessionId: string) => {
      const ctx = sessionContexts.get(sessionId);
      if (!ctx?.fallbackReplyTimer) return;
      clearTimeout(ctx.fallbackReplyTimer);
      ctx.fallbackReplyTimer = null;
    };

    const invokeFeishuSend = async (threadId: string, text: string) => {
      await invoke("send_feishu_text_message", {
        chatId: threadId,
        text,
        appId: null,
        appSecret: null,
        sidecarBaseUrl: null,
      });
    };

    const invokeWecomSend = async (threadId: string, text: string) => {
      await invoke("send_wecom_text_message", {
        conversation_id: threadId,
        text,
        sidecar_base_url: null,
      });
    };

    const scheduleFeishuRetry = (threadId: string, text: string, attempt: number, lastError: unknown) => {
      const key = buildChannelRetryKey("feishu", threadId, text);
      if (attempt > FEISHU_MAX_ATTEMPTS) {
        clearFeishuRetryTimer(key);
        console.error(
          "飞书消息转发失败，已降级为仅桌面可见",
          threadId,
          extractErrorMessage(lastError, "unknown error"),
        );
        return;
      }
      if (feishuRetryTimers.has(key)) return;
      const delay =
        FEISHU_RETRY_DELAYS_MS[Math.max(0, attempt - 2)] ??
        FEISHU_RETRY_DELAYS_MS[FEISHU_RETRY_DELAYS_MS.length - 1];
      const timer = setTimeout(() => {
        feishuRetryTimers.delete(key);
        void (async () => {
          try {
            await invokeFeishuSend(threadId, text);
          } catch (error) {
            scheduleFeishuRetry(threadId, text, attempt + 1, error);
          }
        })();
      }, delay);
      feishuRetryTimers.set(key, timer);
    };

    const sendTextToFeishu = async (threadId: string, text: string) => {
      const chatId = threadId.trim();
      const messageText = text.trim().slice(0, 1800);
      if (!chatId || !messageText) return;
      const key = buildChannelRetryKey("feishu", chatId, messageText);
      clearFeishuRetryTimer(key);
      try {
        await invokeFeishuSend(chatId, messageText);
      } catch (error) {
        scheduleFeishuRetry(chatId, messageText, 2, error);
      }
    };

    const sendTextToImThread = async (sourceChannel: string, threadId: string, text: string) => {
      const normalizedChannel = (sourceChannel || "app").trim().toLowerCase();
      const targetThreadId = threadId.trim();
      const messageText = text.trim().slice(0, 1800);
      if (!targetThreadId || !messageText) return;
      if (shouldSuppressOutboundDuplicate(normalizedChannel, targetThreadId, messageText)) {
        return;
      }

      if (normalizedChannel === "wecom") {
        await invokeWecomSend(targetThreadId, messageText);
        return;
      }

      if (normalizedChannel !== "feishu") {
        return;
      }

      await sendTextToFeishu(targetThreadId, messageText);
    };

    const flushImStream = async (sessionId: string, options?: { force?: boolean }) => {
      const ctx = sessionContexts.get(sessionId);
      if (!ctx) return;
      if (ctx.streamFlushInFlight) return;
      const force = Boolean(options?.force);
      const chunk = ctx.streamBuffer.trim();
      if (!chunk) return;
      if (!force) {
        const elapsed = Date.now() - ctx.lastStreamFlushAt;
        if (elapsed < STREAM_FLUSH_INTERVAL_MS) {
          scheduleImStreamFlush(sessionId, STREAM_FLUSH_INTERVAL_MS - elapsed);
          return;
        }
      }
      if (ctx.streamFlushTimer) {
        clearTimeout(ctx.streamFlushTimer);
        ctx.streamFlushTimer = null;
      }
      ctx.streamBuffer = "";
      ctx.streamFlushInFlight = true;
      ctx.lastStreamFlushAt = Date.now();
      try {
        if (chunk.length <= 1800) {
          await sendTextToImThread(
            ctx.sourceChannel,
            ctx.threadId,
            formatFeishuRoleMessage(ctx.roleName, chunk),
          );
          ctx.streamSentCount += 1;
          return;
        }
        let start = 0;
        while (start < chunk.length) {
          const part = chunk.slice(start, start + 1800);
          await sendTextToImThread(
            ctx.sourceChannel,
            ctx.threadId,
            formatFeishuRoleMessage(ctx.roleName, part),
          );
          ctx.streamSentCount += 1;
          start += 1800;
        }
      } finally {
        const latest = sessionContexts.get(sessionId);
        if (!latest) return;
        latest.streamFlushInFlight = false;
        if (latest.streamBuffer.trim().length > 0) {
          const elapsed = Date.now() - latest.lastStreamFlushAt;
          const delayMs = Math.max(0, STREAM_FLUSH_INTERVAL_MS - elapsed);
          scheduleImStreamFlush(sessionId, delayMs);
        }
      }
    };

    const unlistenDispatchPromise = listen<ImRoleDispatchRequest>("im-role-dispatch-request", async ({ payload }) => {
      const cleanedPrompt = sanitizeInboundPrompt(payload.prompt || "");
      const dispatchPrompt = cleanedPrompt || (payload.prompt || "").trim();
      const messageKey = (payload.message_id || "").trim();
      const key = messageKey || `${payload.session_id}|${payload.role_id}|${dispatchPrompt}`;
      if (seen.has(key)) return;
      seen.add(key);

      const existing = sessionContexts.get(payload.session_id);
      const primaryRoleName = payload.role_name || payload.role_id;
      const ctx: ImBridgeSessionContext = {
        threadId: payload.thread_id,
        sourceChannel: (payload.source_channel || existing?.sourceChannel || "app").trim() || "app",
        primaryRoleName,
        roleName: existing?.roleName || primaryRoleName,
        streamBuffer: existing?.streamBuffer ?? "",
        streamSentCount: 0,
        waitingForAnswer: existing?.waitingForAnswer ?? false,
        streamFlushTimer: existing?.streamFlushTimer ?? null,
        fallbackReplyTimer: existing?.fallbackReplyTimer ?? null,
        lastStreamFlushAt: existing?.lastStreamFlushAt ?? 0,
        streamFlushInFlight: existing?.streamFlushInFlight ?? false,
      };
      ctx.primaryRoleName = primaryRoleName;
      if (!ctx.roleName.trim()) {
        ctx.roleName = primaryRoleName;
      }
      sessionContexts.set(payload.session_id, ctx);
      markImManagedSession(payload.session_id);

      try {
        if (ctx.waitingForAnswer) {
          ctx.waitingForAnswer = false;
          await invoke("answer_user_question", { answer: dispatchPrompt });
        } else {
          await invoke("send_message", {
            request: {
              sessionId: payload.session_id,
              parts: [{ type: "text", text: dispatchPrompt }],
            },
          });
        }

        await flushImStream(payload.session_id, { force: true });
        if (ctx.streamSentCount === 0) {
          clearImFallbackReplyTimer(payload.session_id);
          ctx.fallbackReplyTimer = setTimeout(() => {
            const latest = sessionContexts.get(payload.session_id);
            if (!latest || latest.streamSentCount > 0) {
              if (latest) {
                latest.fallbackReplyTimer = null;
              }
              return;
            }
            latest.fallbackReplyTimer = null;
            void (async () => {
              const messages = await invoke<Message[]>("get_messages", {
                sessionId: payload.session_id,
              });
              const latestAssistant = [...messages]
                .reverse()
                .find((m) => m.role === "assistant" && m.content?.trim().length > 0);
              if (latestAssistant) {
                await sendTextToImThread(
                  latest.sourceChannel,
                  latest.threadId,
                  formatFeishuRoleMessage(latest.roleName, latestAssistant.content.slice(0, 1800)),
                );
              }
            })();
          }, 1200);
        }
      } catch (error) {
        console.error("IM 分发执行失败:", error);
        void reportFrontendDiagnostic({
          kind: "im_role_dispatch_failed",
          message: extractErrorMessage(error, "IM 分发执行失败"),
          href: typeof window !== "undefined" ? window.location?.href : undefined,
        });
      } finally {
        setTimeout(() => seen.delete(key), 30_000);
      }
    });

    const unlistenStreamPromise = listen<{
      session_id: string;
      token: string;
      done: boolean;
      sub_agent?: boolean;
      role_id?: string;
      role_name?: string;
    }>("stream-token", ({ payload }) => {
      const ctx = sessionContexts.get(payload.session_id);
      if (!ctx) return;
      clearImFallbackReplyTimer(payload.session_id);
      if (payload.done) {
        void flushImStream(payload.session_id, { force: true });
        return;
      }
      if (payload.sub_agent) {
        const delegatedRole = (payload.role_name || payload.role_id || "").trim();
        if (delegatedRole) {
          if (ctx.roleName !== delegatedRole && ctx.streamBuffer.trim().length > 0) {
            void flushImStream(payload.session_id, { force: true });
          }
          ctx.roleName = delegatedRole;
        }
      } else if (ctx.roleName !== ctx.primaryRoleName) {
        if (ctx.streamBuffer.trim().length > 0) {
          void flushImStream(payload.session_id, { force: true });
        }
        ctx.roleName = ctx.primaryRoleName;
      }
      ctx.streamBuffer += payload.token || "";
      if (ctx.streamBuffer.length >= STREAM_CHUNK_SIZE) {
        void flushImStream(payload.session_id);
      } else {
        scheduleImStreamFlush(payload.session_id, STREAM_FLUSH_INTERVAL_MS);
      }
    });

    const unlistenAskUserPromise = listen<{
      session_id: string;
      question: string;
      options: string[];
    }>("ask-user-event", ({ payload }) => {
      const ctx = sessionContexts.get(payload.session_id);
      if (!ctx) return;
      ctx.waitingForAnswer = true;
      const optionsText = payload.options?.length ? `\n可选项：${payload.options.join(" / ")}` : "";
      void (async () => {
        await flushImStream(payload.session_id, { force: true });
        await sendTextToImThread(
          ctx.sourceChannel,
          ctx.threadId,
          formatFeishuRoleMessage(
            ctx.roleName,
            `${payload.question}${optionsText}\n请直接回复你的选择或补充信息。`,
          ),
        );
      })();
    });

    return () => {
      sessionContexts.forEach((ctx) => {
        if (ctx.streamFlushTimer) {
          clearTimeout(ctx.streamFlushTimer);
          ctx.streamFlushTimer = null;
        }
        if (ctx.fallbackReplyTimer) {
          clearTimeout(ctx.fallbackReplyTimer);
          ctx.fallbackReplyTimer = null;
        }
      });
      feishuRetryTimers.forEach((timer) => clearTimeout(timer));
      feishuRetryTimers.clear();
      unlistenDispatchPromise.then((fn) => fn());
      unlistenStreamPromise.then((fn) => fn());
      unlistenAskUserPromise.then((fn) => fn());
    };
  }, [setImManagedSessionIds]);
}

import { useCallback, useEffect, useState } from "react";

import { openExternalUrl } from "../../utils/openExternalUrl";

type ChatLinkToastState = {
  variant: "success" | "error";
  message: string;
  url: string;
};

export function useChatLinkActions() {
  const [copiedAssistantMessageKey, setCopiedAssistantMessageKey] = useState<string | null>(null);
  const [chatLinkToast, setChatLinkToast] = useState<ChatLinkToastState | null>(null);

  useEffect(() => {
    if (chatLinkToast?.variant !== "success") {
      return;
    }
    const timer = window.setTimeout(() => {
      setChatLinkToast((current) => (current?.variant === "success" ? null : current));
    }, 1200);
    return () => window.clearTimeout(timer);
  }, [chatLinkToast]);

  const handleCopyAssistantMessage = useCallback(async (messageKey: string, content: string) => {
    const trimmed = content.trim();
    if (!trimmed) return;
    await globalThis.navigator?.clipboard?.writeText?.(trimmed);
    setCopiedAssistantMessageKey(messageKey);
    window.setTimeout(() => {
      setCopiedAssistantMessageKey((current) => (current === messageKey ? null : current));
    }, 1600);
  }, []);

  const handleOpenChatExternalLink = useCallback(async (url: string) => {
    try {
      await openExternalUrl(url);
      setChatLinkToast({
        variant: "success",
        message: "已在浏览器打开",
        url,
      });
    } catch (error) {
      console.error("打开会话外链失败:", error);
      setChatLinkToast({
        variant: "error",
        message: "链接打开失败",
        url,
      });
    }
  }, []);

  const handleCopyChatLink = useCallback(async (url: string) => {
    const trimmed = url.trim();
    if (!trimmed) return;
    try {
      await globalThis.navigator?.clipboard?.writeText?.(trimmed);
      setChatLinkToast({
        variant: "success",
        message: "链接已复制",
        url: trimmed,
      });
    } catch (error) {
      console.error("复制会话外链失败:", error);
      setChatLinkToast({
        variant: "error",
        message: "复制链接失败",
        url: trimmed,
      });
    }
  }, []);

  return {
    copiedAssistantMessageKey,
    chatLinkToast,
    handleCopyAssistantMessage,
    handleOpenChatExternalLink,
    handleCopyChatLink,
    closeChatLinkToast: () => setChatLinkToast(null),
  };
}

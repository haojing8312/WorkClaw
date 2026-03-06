import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SettingsView } from "../SettingsView";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("SettingsView translation preferences", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string, payload?: any) => {
      if (command === "list_model_configs") {
        return Promise.resolve([
          {
            id: "model-1",
            name: "Translator Model",
            api_format: "openai",
            base_url: "https://api.openai.com/v1",
            model_name: "gpt-4o-mini",
            is_default: true,
          },
        ]);
      }
      if (command === "list_mcp_servers") return Promise.resolve([]);
      if (command === "list_search_configs") return Promise.resolve([]);
      if (command === "list_provider_configs") return Promise.resolve([]);
      if (command === "get_runtime_preferences") {
        return Promise.resolve({
          default_work_dir: "E:\\workspace",
          default_language: "zh-CN",
          immersive_translation_enabled: true,
          immersive_translation_display: "translated_only",
          immersive_translation_trigger: "auto",
          translation_engine: "model_then_free",
          translation_model_id: "",
        });
      }
      if (command === "set_runtime_preferences") {
        return Promise.resolve({
          default_work_dir: payload?.input?.default_work_dir ?? "E:\\workspace",
          default_language: payload?.input?.default_language ?? "zh-CN",
          immersive_translation_enabled: payload?.input?.immersive_translation_enabled ?? true,
          immersive_translation_display:
            payload?.input?.immersive_translation_display ?? "translated_only",
          immersive_translation_trigger:
            payload?.input?.immersive_translation_trigger ?? "auto",
          translation_engine: payload?.input?.translation_engine ?? "model_then_free",
          translation_model_id: payload?.input?.translation_model_id ?? "",
        });
      }
      return Promise.resolve(null);
    });
  });

  test("settings can load and save default language + immersive translation", async () => {
    render(<SettingsView onClose={() => {}} />);

    fireEvent.click(await screen.findByRole("button", { name: "桌面 / 系统" }));

    const languageSelect = await screen.findByRole("combobox", { name: "默认语言" });
    expect(languageSelect).toHaveValue("zh-CN");

    fireEvent.change(languageSelect, { target: { value: "en-US" } });
    fireEvent.click(screen.getByRole("checkbox", { name: "启用沉浸式翻译" }));
    fireEvent.change(screen.getByRole("combobox", { name: "翻译显示模式" }), {
      target: { value: "bilingual_inline" },
    });
    fireEvent.change(screen.getByRole("combobox", { name: "翻译触发方式" }), {
      target: { value: "manual" },
    });
    fireEvent.change(screen.getByRole("combobox", { name: "翻译引擎策略" }), {
      target: { value: "model_only" },
    });
    fireEvent.change(screen.getByRole("combobox", { name: "翻译模型" }), {
      target: { value: "model-1" },
    });
    fireEvent.click(screen.getByRole("button", { name: "保存语言与翻译设置" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_runtime_preferences", {
        input: expect.objectContaining({
          default_language: "en-US",
          immersive_translation_enabled: false,
          immersive_translation_display: "bilingual_inline",
          immersive_translation_trigger: "manual",
          translation_engine: "model_only",
          translation_model_id: "model-1",
        }),
      });
    });
  });
});

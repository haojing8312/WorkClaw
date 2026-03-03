import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { ClawhubSkillSummary, SkillManifest } from "../types";
import { RiskConfirmDialog } from "./RiskConfirmDialog";

type InstallMode = "skillpack" | "local" | "clawhub" | "industry";

interface Props {
  onInstalled: (skillId: string, options?: { createSession?: boolean }) => void;
  onClose: () => void;
}

export function InstallDialog({ onInstalled, onClose }: Props) {
  const [mode, setMode] = useState<InstallMode>("skillpack");
  const [packPath, setPackPath] = useState("");
  const [username, setUsername] = useState("");
  const [localDir, setLocalDir] = useState("");
  const [industryPath, setIndustryPath] = useState("");
  const [industryCheckMessage, setIndustryCheckMessage] = useState("");
  const [industryChecking, setIndustryChecking] = useState(false);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [mcpWarning, setMcpWarning] = useState<string[]>([]);
  const [clawhubQuery, setClawhubQuery] = useState("");
  const [clawhubLoading, setClawhubLoading] = useState(false);
  const [clawhubResults, setClawhubResults] = useState<ClawhubSkillSummary[]>([]);
  const [selectedClawhubSlug, setSelectedClawhubSlug] = useState<string>("");
  const [installConfirmOpen, setInstallConfirmOpen] = useState(false);

  async function pickFile() {
    const f = await open({ filters: [{ name: "SkillPack", extensions: ["skillpack"] }] });
    if (f && typeof f === "string") setPackPath(f);
  }

  async function pickDir() {
    const d = await open({ directory: true });
    if (d && typeof d === "string") setLocalDir(d);
  }

  async function pickIndustryFile() {
    const f = await open({ filters: [{ name: "IndustryPack", extensions: ["industrypack"] }] });
    if (f && typeof f === "string") setIndustryPath(f);
  }

  function switchMode(m: InstallMode) {
    setMode(m);
    setError("");
    setMcpWarning([]);
    setInstallConfirmOpen(false);
    if (m !== "industry") {
      setIndustryCheckMessage("");
      setIndustryChecking(false);
    }
  }

  async function searchClawhub() {
    const q = clawhubQuery.trim();
    if (!q) {
      setClawhubResults([]);
      setSelectedClawhubSlug("");
      return;
    }
    setClawhubLoading(true);
    setError("");
    try {
      const results = await invoke<ClawhubSkillSummary[]>("search_clawhub_skills", {
        query: q,
        page: 1,
        limit: 20,
      });
      setClawhubResults(results);
      setSelectedClawhubSlug(results[0]?.slug ?? "");
    } catch (e: unknown) {
      setError(String(e));
      setClawhubResults([]);
      setSelectedClawhubSlug("");
    } finally {
      setClawhubLoading(false);
    }
  }

  async function handleIndustryCheckUpdate() {
    if (!industryPath || industryChecking) return;
    setIndustryChecking(true);
    setError("");
    setIndustryCheckMessage("");
    try {
      const result = await invoke<{ message: string }>("check_industry_bundle_update", {
        bundlePath: industryPath,
      });
      setIndustryCheckMessage(result.message);
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setIndustryChecking(false);
    }
  }

  async function handleInstall() {
    setError("");
    setMcpWarning([]);
    setLoading(true);

    try {
      if (mode === "skillpack") {
        if (!packPath || !username.trim()) {
          setError("请选择文件并填写用户名");
          setLoading(false);
          return;
        }
        const manifest = await invoke<SkillManifest>("install_skill", { packPath, username });
        onInstalled(manifest.id);
        onClose();
      } else if (mode === "local") {
        if (!localDir) {
          setError("请选择包含 SKILL.md 的目录");
          setLoading(false);
          return;
        }
        const result = await invoke<{ manifest: { id: string }; missing_mcp: string[] }>("import_local_skill", {
          dirPath: localDir,
        });

        if (result.missing_mcp.length > 0) {
          setMcpWarning(result.missing_mcp);
          onInstalled(result.manifest.id);
          return;
        }

        onInstalled(result.manifest.id);
        onClose();
      } else if (mode === "clawhub") {
        const skill = clawhubResults.find((item) => item.slug === selectedClawhubSlug);
        if (!skill) {
          setError("请先搜索并选择要安装的 ClawHub Skill");
          setLoading(false);
          return;
        }
        const result = await invoke<{ manifest: { id: string }; missing_mcp: string[] }>(
          "install_clawhub_skill",
          { slug: skill.slug, githubUrl: skill.github_url ?? skill.source_url ?? null }
        );
        if (result.missing_mcp.length > 0) {
          setMcpWarning(result.missing_mcp);
          onInstalled(result.manifest.id);
          return;
        }
        onInstalled(result.manifest.id);
        onClose();
      } else {
        if (!industryPath) {
          setError("请选择 .industrypack 文件");
          setLoading(false);
          return;
        }
        const result = await invoke<{
          pack_id: string;
          version: string;
          installed_skills: { id: string; name: string }[];
          missing_mcp: string[];
        }>("install_industry_bundle", { bundlePath: industryPath, installRoot: null });
        if (result.missing_mcp.length > 0) {
          setMcpWarning(result.missing_mcp);
        }
        if (result.installed_skills.length > 0) {
          onInstalled(result.installed_skills[0].id, { createSession: false });
        }
        onClose();
      }
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  function requestInstall() {
    if (loading) return;
    setInstallConfirmOpen(true);
  }

  function handleCancelInstallConfirm() {
    if (loading) return;
    setInstallConfirmOpen(false);
  }

  function handleConfirmInstall() {
    if (loading) return;
    setInstallConfirmOpen(false);
    void handleInstall();
  }

  const selectedClawhubSkill = clawhubResults.find((item) => item.slug === selectedClawhubSlug);
  const installRiskSummary = mode === "skillpack"
    ? "确认安装该 .skillpack 技能包吗？"
    : mode === "local"
    ? "确认导入该本地技能目录吗？"
    : mode === "clawhub"
    ? `确认安装「${selectedClawhubSkill?.name ?? "所选 ClawHub 技能"}」吗？`
    : "确认安装该行业包吗？";
  const installRiskImpact = mode === "skillpack"
    ? (packPath ? `文件: ${packPath}` : "请先选择 .skillpack 文件并填写用户名。")
    : mode === "local"
    ? (localDir ? `目录: ${localDir}` : "请先选择本地技能目录。")
    : mode === "clawhub"
    ? (selectedClawhubSkill ? `slug: ${selectedClawhubSkill.slug}` : "请先搜索并选择要安装的 ClawHub 技能。")
    : (industryPath ? `文件: ${industryPath}` : "请先选择 .industrypack 文件。");

  const tabBase = "flex-1 py-1.5 text-sm rounded transition-colors text-center";
  const tabActive = "bg-blue-500 text-white";
  const tabInactive = "bg-gray-100 text-gray-500 hover:bg-gray-200";

  return (
    <div className="fixed inset-0 bg-black/30 backdrop-blur-sm flex items-center justify-center z-50">
      <div className="bg-white rounded-lg p-6 w-96 space-y-4 border border-gray-200 shadow-xl">
        <h2 className="font-semibold text-lg text-gray-900">安装 Skill</h2>

        <div className="grid grid-cols-2 gap-2">
          <button
            className={`${tabBase} ${mode === "skillpack" ? tabActive : tabInactive}`}
            onClick={() => switchMode("skillpack")}
          >
            加密 .skillpack
          </button>
          <button
            className={`${tabBase} ${mode === "local" ? tabActive : tabInactive}`}
            onClick={() => switchMode("local")}
          >
            本地目录
          </button>
          <button
            className={`${tabBase} ${mode === "clawhub" ? tabActive : tabInactive}`}
            onClick={() => switchMode("clawhub")}
          >
            ClawHub
          </button>
          <button
            className={`${tabBase} ${mode === "industry" ? tabActive : tabInactive}`}
            onClick={() => switchMode("industry")}
          >
            行业包
          </button>
        </div>

        {mode === "skillpack" && (
          <>
            <div>
              <button
                onClick={pickFile}
                className="w-full border border-dashed border-gray-300 rounded p-3 text-sm text-gray-500 hover:border-blue-400 hover:text-blue-500 transition-colors"
              >
                {packPath ? packPath.split(/[/\\]/).pop() : "选择 .skillpack 文件"}
              </button>
            </div>
            <div>
              <label className="block text-xs text-gray-500 mb-1">用户名（创作者提供）</label>
              <input
                className="w-full bg-gray-50 border border-gray-200 rounded px-3 py-2 text-sm focus:outline-none focus:border-blue-400 focus:ring-1 focus:ring-blue-400"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                placeholder=""
              />
            </div>
          </>
        )}

        {mode === "local" && (
          <>
            <div>
              <button
                onClick={pickDir}
                className="w-full border border-dashed border-gray-300 rounded p-3 text-sm text-gray-500 hover:border-blue-400 hover:text-blue-500 transition-colors"
              >
                {localDir ? localDir.split(/[/\\]/).pop() : "选择 Skill 目录"}
              </button>
              {localDir && (
                <div className="mt-1 text-xs text-gray-400 truncate" title={localDir}>
                  {localDir}
                </div>
              )}
            </div>
            <div className="text-xs text-gray-400">
              目录中需包含 <code className="text-gray-500">SKILL.md</code> 文件。
              本地 Skill 无需加密，可直接导入使用。
            </div>
          </>
        )}

        {mode === "clawhub" && (
          <div className="space-y-2">
            <div className="flex gap-2">
              <input
                className="flex-1 bg-gray-50 border border-gray-200 rounded px-3 py-2 text-sm focus:outline-none focus:border-blue-400 focus:ring-1 focus:ring-blue-400"
                value={clawhubQuery}
                onChange={(e) => setClawhubQuery(e.target.value)}
                placeholder="输入关键词搜索 ClawHub 技能"
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    void searchClawhub();
                  }
                }}
              />
              <button
                onClick={() => void searchClawhub()}
                disabled={clawhubLoading}
                className="px-3 rounded bg-blue-50 hover:bg-blue-100 disabled:bg-gray-100 text-blue-700 text-xs"
              >
                {clawhubLoading ? "搜索中..." : "搜索"}
              </button>
            </div>

            {clawhubResults.length > 0 ? (
              <div className="max-h-48 overflow-auto border border-gray-200 rounded">
                {clawhubResults.map((skill) => (
                  <button
                    key={skill.slug}
                    onClick={() => setSelectedClawhubSlug(skill.slug)}
                    className={`w-full text-left px-3 py-2 border-b border-gray-100 last:border-b-0 ${
                      selectedClawhubSlug === skill.slug ? "bg-blue-50" : "hover:bg-gray-50"
                    }`}
                  >
                    <div className="text-sm text-gray-800 font-medium truncate">{skill.name}</div>
                    <div className="text-[11px] text-gray-500 truncate">
                      {skill.description || "暂无描述"}
                    </div>
                    <div className="text-[10px] text-gray-400 mt-1">
                      slug: {skill.slug} · stars: {skill.stars ?? 0}
                    </div>
                  </button>
                ))}
              </div>
            ) : (
              <div className="text-xs text-gray-400">通过关键字搜索 ClawHub 公共技能后可直接安装。</div>
            )}
          </div>
        )}

        {mode === "industry" && (
          <div className="space-y-2">
            <button
              onClick={pickIndustryFile}
              className="w-full border border-dashed border-gray-300 rounded p-3 text-sm text-gray-500 hover:border-blue-400 hover:text-blue-500 transition-colors"
            >
              {industryPath ? industryPath.split(/[/\\]/).pop() : "选择 .industrypack 文件"}
            </button>
            {industryPath && (
              <div className="text-xs text-gray-400 truncate" title={industryPath}>
                {industryPath}
              </div>
            )}
            <button
              onClick={() => void handleIndustryCheckUpdate()}
              disabled={!industryPath || industryChecking}
              className="h-7 px-3 rounded bg-blue-50 hover:bg-blue-100 disabled:bg-gray-100 text-blue-700 text-xs transition-colors"
            >
              {industryChecking ? "检查中..." : "检查更新"}
            </button>
            {industryCheckMessage && (
              <div className="text-xs text-amber-700 bg-amber-50 border border-amber-100 rounded p-2">
                {industryCheckMessage}
              </div>
            )}
          </div>
        )}

        {error && <div className="text-red-500 text-sm">{error}</div>}

        {mcpWarning.length > 0 && (
          <div className="text-amber-600 text-sm">
            <div className="font-medium mb-1">此 Skill 需要以下 MCP 服务器：</div>
            <ul className="list-disc list-inside">
              {mcpWarning.map((name) => (
                <li key={name} className="text-xs">
                  {name}
                </li>
              ))}
            </ul>
            <div className="text-xs text-gray-400 mt-1">请在设置 → MCP 服务器中配置</div>
          </div>
        )}

        <div className="flex gap-2">
          <button
            onClick={onClose}
            className="flex-1 bg-gray-100 hover:bg-gray-200 active:scale-[0.97] text-gray-700 py-2 rounded-lg text-sm transition-all"
          >
            取消
          </button>
          <button
            onClick={requestInstall}
            disabled={loading}
            className="flex-1 bg-blue-500 hover:bg-blue-600 active:scale-[0.97] disabled:bg-gray-200 disabled:text-gray-400 text-white py-2 rounded-lg text-sm transition-all"
          >
            {loading ? "安装中..." : "安装"}
          </button>
        </div>
      </div>
      <RiskConfirmDialog
        open={installConfirmOpen}
        level="medium"
        title="安装技能"
        summary={installRiskSummary}
        impact={installRiskImpact}
        irreversible={false}
        confirmLabel="确认安装"
        cancelLabel="取消"
        loading={loading}
        onConfirm={handleConfirmInstall}
        onCancel={handleCancelInstallConfirm}
      />
    </div>
  );
}

import { useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { FrontMatter } from "../../types";

interface WorkClawDirSummary {
  dir_path: string;
  slug: string;
  front_matter: FrontMatter;
  tags: string[];
}

const ALL_TAG_OPTION = "全部标签";

function getPathLeaf(path: string) {
  return path.split(/[/\\]/).filter(Boolean).pop() ?? "";
}

export function IndustryPackView() {
  const [rootDir, setRootDir] = useState("");
  const [skills, setSkills] = useState<WorkClawDirSummary[]>([]);
  const [selectedMap, setSelectedMap] = useState<Record<string, boolean>>({});
  const [tagDraftMap, setTagDraftMap] = useState<Record<string, string>>({});
  const [packName, setPackName] = useState("");
  const [packId, setPackId] = useState("");
  const [version, setVersion] = useState("1.0.0");
  const [industryTag, setIndustryTag] = useState("");
  const [filterTag, setFilterTag] = useState(ALL_TAG_OPTION);
  const [status, setStatus] = useState<"idle" | "packing" | "done" | "error">("idle");
  const [message, setMessage] = useState("");
  const packInFlightRef = useRef(false);

  const tagOptions = useMemo(() => {
    const tags = new Set<string>();
    for (const skill of skills) {
      for (const tag of skill.tags ?? []) {
        if (tag.trim()) tags.add(tag.trim());
      }
    }
    return [ALL_TAG_OPTION, ...Array.from(tags)];
  }, [skills]);

  const visibleSkills = useMemo(() => {
    if (filterTag === ALL_TAG_OPTION) return skills;
    return skills.filter((item) => (item.tags ?? []).includes(filterTag));
  }, [skills, filterTag]);

  const selectedCount = useMemo(
    () => skills.filter((item) => selectedMap[item.dir_path]).length,
    [selectedMap, skills]
  );

  function setAllSelections(checked: boolean) {
    setSelectedMap(() => {
      const nextSelected: Record<string, boolean> = {};
      for (const skill of skills) {
        nextSelected[skill.dir_path] = checked;
      }
      return nextSelected;
    });
  }

  async function handlePickRootDir() {
    const selected = await open({ directory: true, multiple: false });
    if (!selected || typeof selected !== "string") return;

    const dirName = getPathLeaf(selected);
    setRootDir(selected);
    setPackName(dirName);
    setPackId(dirName);
    setFilterTag(ALL_TAG_OPTION);
    setStatus("idle");
    setMessage("");
    try {
      const list = await invoke<WorkClawDirSummary[]>("scan_workclaw_dirs", {
        rootDir: selected,
      });
      const nextSelected: Record<string, boolean> = {};
      const nextDrafts: Record<string, string> = {};
      for (const row of list) {
        nextSelected[row.dir_path] = true;
        nextDrafts[row.dir_path] = (row.tags ?? []).join(", ");
      }
      setSkills(list);
      setSelectedMap(nextSelected);
      setTagDraftMap(nextDrafts);
    } catch (e: unknown) {
      setSkills([]);
      setSelectedMap({});
      setTagDraftMap({});
      setStatus("error");
      setMessage(String(e));
    }
  }

  async function handleSaveTags(skill: WorkClawDirSummary) {
    const raw = tagDraftMap[skill.dir_path] ?? "";
    const tags = raw
      .split(",")
      .map((item) => item.trim())
      .filter(Boolean);
    await invoke("update_skill_dir_tags", {
      dirPath: skill.dir_path,
      tags,
    });
    setSkills((prev) =>
      prev.map((item) =>
        item.dir_path === skill.dir_path ? { ...item, tags } : item
      )
    );
  }

  async function handlePack() {
    if (packInFlightRef.current || status === "packing") return;
    const selectedSkillDirs = skills
      .filter((item) => selectedMap[item.dir_path])
      .map((item) => item.dir_path);
    if (selectedSkillDirs.length === 0) {
      setStatus("error");
      setMessage("请至少选择一个技能");
      return;
    }
    if (!packName.trim()) {
      setStatus("error");
      setMessage("请填写行业包名称");
      return;
    }
    if (!packId.trim()) {
      setStatus("error");
      setMessage("请填写包 ID");
      return;
    }

    packInFlightRef.current = true;
    const outputPath = await save({
      defaultPath: `${packId.trim()}.industrypack`,
      filters: [{ name: "IndustryPack", extensions: ["industrypack"] }],
    });
    if (!outputPath) {
      packInFlightRef.current = false;
      return;
    }

    setStatus("packing");
    setMessage("");
    try {
      await invoke("pack_industry_bundle", {
        skillDirs: selectedSkillDirs,
        packName,
        packId,
        version,
        industryTag,
        outputPath,
      });
      setStatus("done");
      setMessage("行业包导出成功");
    } catch (e: unknown) {
      setStatus("error");
      setMessage(String(e));
    } finally {
      packInFlightRef.current = false;
    }
  }

  const inputCls =
    "w-full bg-gray-50 border border-gray-200 rounded-md px-3 py-2 text-sm text-gray-900 focus:outline-none focus:border-blue-500 focus:ring-1 focus:ring-blue-500/30 transition-colors";

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-2">
        <button
          onClick={handlePickRootDir}
          className="bg-blue-500 hover:bg-blue-600 text-sm px-4 py-1.5 rounded-md font-medium text-white transition-colors"
        >
          选择技能根目录
        </button>
        {rootDir && (
          <span className="text-xs text-gray-500 truncate" title={rootDir}>
            {rootDir}
          </span>
        )}
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
        <div>
          <label htmlFor="industry-pack-name" className="block text-xs font-medium text-gray-500 mb-1.5">
            行业包名称
          </label>
          <input
            id="industry-pack-name"
            className={inputCls}
            value={packName}
            onChange={(e) => setPackName(e.target.value)}
            placeholder="例如：教师行业包"
          />
        </div>
        <div>
          <label htmlFor="industry-pack-id" className="block text-xs font-medium text-gray-500 mb-1.5">
            包 ID
          </label>
          <input
            id="industry-pack-id"
            className={inputCls}
            value={packId}
            onChange={(e) => setPackId(e.target.value)}
            placeholder="例如：edu-teacher-suite"
          />
        </div>
        <div>
          <label htmlFor="industry-pack-version" className="block text-xs font-medium text-gray-500 mb-1.5">
            版本
          </label>
          <input
            id="industry-pack-version"
            className={inputCls}
            value={version}
            onChange={(e) => setVersion(e.target.value)}
          />
        </div>
        <div>
          <label htmlFor="industry-tag" className="block text-xs font-medium text-gray-500 mb-1.5">
            行业标签
          </label>
          <input
            id="industry-tag"
            className={inputCls}
            value={industryTag}
            onChange={(e) => setIndustryTag(e.target.value)}
            placeholder="例如：教师"
          />
        </div>
      </div>

      <div className="rounded-lg border border-gray-200 bg-gray-50 px-3 py-2">
        <div className="flex flex-col gap-2 md:flex-row md:items-center md:justify-between">
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-xs font-medium text-gray-500">标签筛选</span>
            {tagOptions.map((tag) => (
              <button
                key={tag}
                onClick={() => setFilterTag(tag)}
                className={`px-2.5 h-7 rounded-full text-xs border transition-colors ${
                  filterTag === tag
                    ? "bg-blue-500 text-white border-blue-500"
                    : "bg-white text-gray-600 border-gray-200 hover:border-blue-300"
                }`}
              >
                {tag}
              </button>
            ))}
          </div>
          {skills.length > 0 && (
            <div className="flex flex-wrap items-center gap-2">
              <span className="text-xs text-gray-500">
                已选 {selectedCount} / {skills.length}
              </span>
              <button
                onClick={() => setAllSelections(true)}
                className="h-7 px-2.5 rounded-md bg-white border border-gray-200 text-xs text-gray-700 hover:border-blue-300 transition-colors"
              >
                全选
              </button>
              <button
                onClick={() => setAllSelections(false)}
                className="h-7 px-2.5 rounded-md bg-white border border-gray-200 text-xs text-gray-700 hover:border-blue-300 transition-colors"
              >
                全不选
              </button>
            </div>
          )}
        </div>
      </div>

      {visibleSkills.length === 0 ? (
        <div className="rounded-xl border border-dashed border-gray-200 bg-white px-4 py-8 text-center text-sm text-gray-400">
          请选择技能根目录并加载 WorkClaw 列表
        </div>
      ) : (
        <div className="space-y-2">
          {visibleSkills.map((skill) => (
            <div key={skill.dir_path} className="rounded-lg border border-gray-200 bg-white p-3">
              <div className="flex items-start gap-3">
                <input
                  type="checkbox"
                  aria-label={`选择 ${skill.front_matter.name || skill.slug}`}
                  checked={Boolean(selectedMap[skill.dir_path])}
                  onChange={(e) =>
                    setSelectedMap((prev) => ({
                      ...prev,
                      [skill.dir_path]: e.target.checked,
                    }))
                  }
                  className="mt-1"
                />
                <div className="flex-1 min-w-0">
                  <div className="text-sm font-medium text-gray-800 truncate">
                    {skill.front_matter.name || skill.slug}
                  </div>
                  <div className="text-xs text-gray-500 truncate mt-0.5">{skill.dir_path}</div>
                  <div className="text-xs text-gray-400 mt-1">
                    版本：{skill.front_matter.version || "1.0.0"}
                  </div>
                  <div className="mt-2 flex items-center gap-2">
                    <input
                      className="flex-1 bg-gray-50 border border-gray-200 rounded px-2 py-1 text-xs focus:outline-none focus:border-blue-400"
                      value={tagDraftMap[skill.dir_path] ?? ""}
                      onChange={(e) =>
                        setTagDraftMap((prev) => ({
                          ...prev,
                          [skill.dir_path]: e.target.value,
                        }))
                      }
                      placeholder="标签，英文逗号分隔"
                    />
                    <button
                      onClick={() => void handleSaveTags(skill)}
                      className="h-7 px-2.5 rounded bg-gray-100 hover:bg-gray-200 text-xs text-gray-700"
                    >
                      保存标签
                    </button>
                  </div>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}

      {status === "error" && message && (
        <div className="text-red-600 text-sm bg-red-50 border border-red-200 rounded-md p-3">{message}</div>
      )}
      {status === "done" && (
        <div className="text-green-700 text-sm bg-green-50 border border-green-200 rounded-md p-3">
          {message}
        </div>
      )}

      <button
        onClick={handlePack}
        disabled={status === "packing"}
        className="w-full bg-blue-500 hover:bg-blue-600 disabled:bg-gray-200 disabled:text-gray-400 text-white font-medium py-2.5 rounded-md transition-colors text-sm"
      >
        {status === "packing" ? "导出中..." : "导出行业包"}
      </button>
    </div>
  );
}
